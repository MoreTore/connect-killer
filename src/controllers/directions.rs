#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{Extension, State}, 
    http::{HeaderMap, StatusCode, Uri, Method}, 
    response::{Response, Result},
    body::{Body, to_bytes},
};

use std::{env, collections::HashMap};
use serde_urlencoded;
use crate::middleware::auth::MyJWT;


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

pub async fn proxy_mapbox(
    _auth: MyJWT,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ErrorResponse> {

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


pub fn routes() -> Routes {
    Routes::new()
        .prefix("maps")
        .add("/*path", axum::routing::any(proxy_mapbox))
}
