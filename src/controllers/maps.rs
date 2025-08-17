#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{Extension, State}, 
    http::{HeaderMap, StatusCode, Uri, Method}, 
    response::{Response, Result},
    body::{Body, to_bytes},
};

use std::time::Duration;
use std::{env, collections::HashMap};
use serde_urlencoded;
use crate::middleware::auth::MyJWT;
use governor::{Quota, DefaultDirectRateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use once_cell::sync::Lazy;
use axum::http::Response as AxumResponse;
use dashmap::DashMap;

static GLOBAL_MINUTE_RATE_LIMITER: Lazy<Arc<DefaultDirectRateLimiter>> = Lazy::new(|| {
    Arc::new(DefaultDirectRateLimiter::new(
        Quota::with_period(Duration::from_secs(60)).unwrap()
        .allow_burst(NonZeroU32::new(100).unwrap()),
        Default::default(), // clock
        Default::default(), // state
    ))
});

static GLOBAL_HOUR_RATE_LIMITER: Lazy<Arc<DefaultDirectRateLimiter>> = Lazy::new(|| {
    Arc::new(DefaultDirectRateLimiter::new(
        Quota::with_period(Duration::from_secs(60 * 60)).unwrap()
        .allow_burst(NonZeroU32::new(500).unwrap()),
        Default::default(), // clock
        Default::default(), // state
    ))
});

static GLOBAL_DAILY_RATE_LIMITER: Lazy<Arc<DefaultDirectRateLimiter>> = Lazy::new(|| {
    Arc::new(DefaultDirectRateLimiter::new(
        Quota::with_period(Duration::from_secs(60 * 60 * 24)).unwrap()
        .allow_burst(NonZeroU32::new(6000).unwrap()),
        Default::default(), // clock
        Default::default(), // state
    ))
});

static GLOBAL_MONTHLY_LIMITER: Lazy<Arc<DefaultDirectRateLimiter>> = Lazy::new(|| {
    Arc::new(DefaultDirectRateLimiter::new(
        Quota::with_period(Duration::from_secs(60 * 60 * 24 * 30)).unwrap()
        .allow_burst(NonZeroU32::new(100_000).unwrap()),
        Default::default(),
        Default::default(),
    ))
});

static USER_LIMITERS: Lazy<DashMap<String, Arc<DefaultDirectRateLimiter>>> = Lazy::new(|| {
    DashMap::new()
});

fn build_user_limiter() -> DefaultDirectRateLimiter {
    DefaultDirectRateLimiter::new(
        Quota::with_period(Duration::from_secs(60 * 60 * 24)).unwrap()
             .allow_burst(NonZeroU32::new(200).unwrap()),
        Default::default(),
        Default::default(),
    )
}


#[derive(Debug)]
pub enum ErrorResponse {
    UpstreamError(String),
    Unauthorized,
    InternalServerError,
}

impl From<reqwest::Error> for ErrorResponse {
    fn from(error: reqwest::Error) -> Self {
        ErrorResponse::UpstreamError(format!("Error from upstream service: {}", error))
    }
}


impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ErrorResponse::UpstreamError(message) => (
                StatusCode::BAD_GATEWAY,
                format!("Upstream service error: {}", message),
            ),
            ErrorResponse::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Unauthorized".to_string(),
            ),
            ErrorResponse::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error occurred".to_string(),
            ),
        };

        Response::builder()
            .status(status)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Body::from(body))
            .unwrap()
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("maps")
        .add("/*path", axum::routing::any(proxy_mapbox))
}


pub async fn proxy_mapbox(
    auth: MyJWT,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    method: Method,
    uri: Uri,
    _headers: HeaderMap,
    body: Body,
) -> Result<Response, ErrorResponse> {
    tracing::trace!("Proxying request to Mapbox API: {}", uri);

    let user_limiter = USER_LIMITERS
        .entry(auth.claims.identity.clone())
        .or_insert_with(|| Arc::new(build_user_limiter()))
        .value()
        .clone();

    if user_limiter.check().is_err() {
        tracing::trace!("User {} exceeded their personal quota", auth.claims.identity);
        return Ok(AxumResponse::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::from("Personal rate limit exceeded. Try again later."))
            .unwrap());
    }

    // Check global rate limits
    let minute_check = GLOBAL_MINUTE_RATE_LIMITER.check().is_err();
    let hour_check = GLOBAL_HOUR_RATE_LIMITER.check().is_err();
    let daily_check = GLOBAL_DAILY_RATE_LIMITER.check().is_err();
    let monthly_check = GLOBAL_MONTHLY_LIMITER.check().is_err();
    
    if minute_check || hour_check || daily_check || monthly_check {
        tracing::trace!("Rate limit exceeded");
        return Ok(AxumResponse::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Body::from("Rate limit exceeded. Try again later."))
            .unwrap());
    }

    let mapbox_token = env::var("MAPBOX_TOKEN").expect("MAPBOX_TOKEN env variable not set");

    let path = uri.path().replacen("/maps", "", 1);
    let mut mapbox_url = format!("https://api.mapbox.com{}", path);

    let mut query_params: HashMap<String, String> = uri.query().map_or_else(HashMap::new, |q| {
        serde_urlencoded::from_str(q).unwrap_or_else(|e| {
            tracing::error!("Error parsing query string: {}", e);
            HashMap::new()
        })
    });

    query_params.insert("access_token".to_string(), mapbox_token);

    let updated_query = serde_urlencoded::to_string(&query_params).unwrap();
    if !updated_query.is_empty() {
        mapbox_url.push_str("?");
        mapbox_url.push_str(&updated_query);
    }
    tracing::debug!("the mapbox url is: {}", mapbox_url);

    let size_limit = 10 * 1024 * 1024;
    let body_bytes = to_bytes(body, size_limit).await.map_err(|_| ErrorResponse::InternalServerError)?;
    let reqwest_body = reqwest::Body::from(body_bytes);

    let request_builder = client
        .request(method, mapbox_url)
        .headers(HeaderMap::new()) // Optional: forward some headers
        .body(reqwest_body);

    let upstream_response = request_builder.send().await?;

    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let body_string = upstream_response.text().await?;
    let body = Body::from(body_string);

    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = headers;

    Ok(response)
}