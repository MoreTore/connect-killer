#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get, Extension,
  
  };
use serde::{Deserialize, Serialize};
use crate::common;


#[derive(Deserialize)]
pub struct UlogQuery {
    pub url: String
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String
}

pub async fn asset_download(
    lookup_key: String,
    client: &reqwest::Client,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Validate the lookup_key to allow only alphanumeric, '_', '-' and '.' characters
    let valid_key_pattern = regex:: Regex::new(r"^[a-zA-Z0-9_.-]+$").unwrap();
    if !valid_key_pattern.is_match(&lookup_key) {
        return Err((StatusCode::BAD_REQUEST, "Invalid lookup key"));
    }
    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&lookup_key);

    // Prepare a request to fetch the file from storage
    let mut request_builder = client.get(&internal_file_url);
    
    // Check for range header and forward it if present
    let range_header_present = if let Some(range) = headers.get(hyper::header::RANGE) {
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
        true
    } else {
        false
    };

    let res = request_builder.send().await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                let content_length = response.headers().get(hyper::header::CONTENT_LENGTH)
                    .and_then(|ct_len| ct_len.to_str().ok())
                    .and_then(|ct_len| ct_len.parse::<u64>().ok());

                let mut response_builder = Response::builder();
                response_builder = response_builder
                    .header(hyper::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{lookup_key}\""));

                // Add Content-Length if available
                if let Some(length) = content_length {
                    response_builder = response_builder.header(hyper::header::CONTENT_LENGTH, length);
                }

                let body = reqwest::Body::wrap_stream(response.bytes_stream());
                if range_header_present {
                    response_builder = response_builder.status(StatusCode::PARTIAL_CONTENT);
                } else {
                    response_builder = response_builder.status(StatusCode::OK);
                }
                let proxy_response = response_builder.body(body).unwrap();

                Ok(proxy_response)
            } else {
                Err((StatusCode::from(response.status()), "Failed to fetch the file"))
            }
        }
        Err(_) => Err((StatusCode::BAD_GATEWAY, "Internal server error")),
    }
}

pub async fn events_download(
    Path((dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--events.json");  // canonical_route_name include dongleid already
    asset_download(lookup_key, &client, headers).await
}

pub async fn coords_download(
    Path((dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--coords.json");  // canonical_route_name include dongleid already
    asset_download(lookup_key, &client, headers).await
}

pub async fn sprite_download(
    Path((dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--sprite.jpg");  // canonical_route_name include dongleid already
    asset_download(lookup_key, &client, headers).await
}

pub async fn auth_file_download(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, route_name, segment, file)): Path<(String, String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{dongle_id}_{route_name}--{segment}--{file}");  // Note that route_name does not include dongle_id
    asset_download(lookup_key, &client, headers).await
}

pub async fn bootlog_file_download(
    auth: crate::middleware::auth::MyJWT,
    Path(bootlog_file): Path<String>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Note that bootlogs path is the lookup key
    asset_download(bootlog_file, &client, headers).await
}

// TODO Migrate DB to remove the redundant file_type path
pub async fn depreciated_auth_file_download(
    auth: crate::middleware::auth::MyJWT,
    Path((_file, dongle_id, route_name, segment, file)): Path<(String, String, String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{dongle_id}_{route_name}--{segment}--{file}");  // Note that route_name does not include dongle_id
    asset_download(lookup_key, &client, headers).await
}

pub async fn render_segment_ulog(
    auth: crate::middleware::auth::MyJWT,
    ViewEngine(v): ViewEngine<TeraView>, 
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    Query(params): Query<UlogQuery>
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Validate the lookup_key to allow only alphanumeric, '_', '-' and '.' characters
    let valid_key_pattern = regex::Regex::new(r"^(http://localhost:3000/|https://localhost:3000/)?([a-zA-Z0-9_.-]+)$").unwrap(); //TODO purge hardcoded url from db
    if let Some(captures) = valid_key_pattern.captures(&params.url) {
        // Always use common::mkv_helpers::get_mkv_file_url with the second part (lookup key)
        let internal_file_url = common::mkv_helpers::get_mkv_file_url(&captures[2]);
    
        // Proceed with the request using the `internal_file_url`
        let request = client.get(&internal_file_url);
    
        // Get the data and save it as a string to pass to admin_segment_ulog
        let res = request.send().await;
    
        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let bytes = response.bytes().await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Error reading response"))?;
                    let bytes_vec: Vec<u8> = bytes.to_vec(); // Convert bytes to Vec<u8>
                    let data = unsafe { String::from_utf8_unchecked(bytes_vec) };
    
                    // Render the view with the fetched data
                    Ok(crate::views::route::admin_segment_ulog(v, UlogText { text: data }))
                } else {
                    Err((StatusCode::BAD_GATEWAY, "Failed to fetch the segment"))
                }
            }
            Err(_) => Err((StatusCode::BAD_GATEWAY, "Failed to fetch the segment")),
        }
    } else {
        Err((StatusCode::BAD_REQUEST, "Invalid lookup key"))
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectdata")
        .add("/:dongle_id/:timestamp/:segment/coords.json", get(coords_download))
        .add("/:dongle_id/:timestamp/:segment/events.json", get(events_download))
        .add("/:dongle_id/:timestamp/:segment/sprite.jpg", get(sprite_download))
        .add("/:dongle_id/:timestamp/:segment/:file", get(auth_file_download))
        .add("/:filetype/:dongle_id/:timestamp/:segment/:file", get(depreciated_auth_file_download))
        .add("/logs/", get(render_segment_ulog))
        .add("/bootlog/:bootlog_file", get(bootlog_file_download))
}
