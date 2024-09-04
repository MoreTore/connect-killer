#![allow(clippy::unused_async)]
use loco_rs::{prelude::*};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get, Extension,
  
  };
use serde::{Deserialize, Serialize};
use crate::common;

// used for comma tools
pub async fn file_stream(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, route_name, segment, file)): Path<(String, String, String, String)>,
    State(_ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap, // Include headers from the incoming request
  ) -> impl IntoResponse {
    let lookup_key = match file.as_str() {
        "sprite.jpg" | "coords.json" => format!("{route_name}--{segment}--{file}"), // route_name already has the dongle_id in this case
        _ => format!("{dongle_id}_{route_name}--{segment}--{file}")
    };
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
        },
        Err(_) => Err((StatusCode::BAD_GATEWAY, "Internal server error")),
    }
  }

// used for useradmin browser download
pub async fn file_download(
    auth: crate::middleware::auth::MyJWT,
    Path((_filetype, dongle_id, timestamp, segment, file)): Path<(String, String, String, String, String)>,
    State(_ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{dongle_id}_{timestamp}--{segment}--{file}");
    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&lookup_key);

    // Prepare a request to fetch the file from storage
    let mut request_builder = client.get(&internal_file_url);

    // Forward the Range header if present
    if let Some(range) = headers.get(hyper::header::RANGE) {
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
    }

    let res = request_builder.send().await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                // Create a response builder to form the browser response
                let mut response_builder = Response::builder()
                    .status(StatusCode::OK)
                    .header(hyper::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{lookup_key}\""));

                // Add Content-Length if available
                if let Some(content_length) = response.headers().get(hyper::header::CONTENT_LENGTH)
                                                            .and_then(|ct_len| ct_len.to_str().ok())
                                                            .and_then(|ct_len| ct_len.parse::<u64>().ok()) {
                    response_builder = response_builder.header(hyper::header::CONTENT_LENGTH, content_length);
                }

                // Add the file content as the response body
                let body = reqwest::Body::wrap_stream(response.bytes_stream());
                let proxy_response = response_builder.body(body).unwrap();

                Ok(proxy_response)
            } else {
                Err((StatusCode::from(response.status()), "Failed to fetch the file"))
            }
        },
        Err(_) => Err((StatusCode::BAD_GATEWAY, "Internal server error")),
    }
}

// used for useradmin browser download
pub async fn bootlog_file_download(
    Path(bootlog_file): Path<String>,
    State(_ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&bootlog_file);

    // Prepare a request to fetch the file from storage
    let mut request_builder = client.get(&internal_file_url);

    // Forward the Range header if present
    if let Some(range) = headers.get(hyper::header::RANGE) {
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
    }

    let res = request_builder.send().await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                // Create a response builder to form the browser response
                let mut response_builder = Response::builder()
                    .status(StatusCode::OK)
                    .header(hyper::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{bootlog_file}\""));

                // Add Content-Length if available
                if let Some(content_length) = response.headers().get(hyper::header::CONTENT_LENGTH)
                                                            .and_then(|ct_len| ct_len.to_str().ok())
                                                            .and_then(|ct_len| ct_len.parse::<u64>().ok()) {
                    response_builder = response_builder.header(hyper::header::CONTENT_LENGTH, content_length);
                }

                // Add the file content as the response body
                let body = reqwest::Body::wrap_stream(response.bytes_stream());
                let proxy_response = response_builder.body(body).unwrap();

                Ok(proxy_response)
            } else {
                Err((StatusCode::from(response.status()), "Failed to fetch the file"))
            }
        },
        Err(_) => Err((StatusCode::BAD_GATEWAY, "Internal server error")),
    }
}

// a2a0ccea32023010/e8d8f1d92f2945750e031414a701cca9_2023-07-27--13-01-19/12/sprite.jpg
pub async fn thumbnail_download(
    //auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, route_name, segment, file)): Path<(String, String, String, String)>,
    State(_ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {

    let lookup_key = match file.as_str() {
        "sprite.jpg" | "coords.json" => format!("{route_name}--{segment}--{file}"), // route_name already has the dongle_id in this case
        _ => format!("{dongle_id}_{route_name}--{segment}--{file}")
    };

    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&lookup_key);

    // Prepare a request to fetch the file from storage
    let mut request_builder = client.get(&internal_file_url);

    // Forward the Range header if present
    if let Some(range) = headers.get(hyper::header::RANGE) {
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
    }

    let res = request_builder.send().await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                // Create a response builder to form the browser response
                let mut response_builder = Response::builder()
                    .status(StatusCode::OK)
                    .header(hyper::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{lookup_key}\""));

                // Add Content-Length if available
                if let Some(content_length) = response.headers().get(hyper::header::CONTENT_LENGTH)
                                                            .and_then(|ct_len| ct_len.to_str().ok())
                                                            .and_then(|ct_len| ct_len.parse::<u64>().ok()) {
                    response_builder = response_builder.header(hyper::header::CONTENT_LENGTH, content_length);
                }

                // Add the file content as the response body
                let body = reqwest::Body::wrap_stream(response.bytes_stream());
                let proxy_response = response_builder.body(body).unwrap();

                Ok(proxy_response)
            } else {
                Err((StatusCode::from(response.status()), "Failed to fetch the file"))
            }
        },
        Err(_) => Err((StatusCode::BAD_GATEWAY, "Internal server error")),
    }
}

#[derive(Deserialize)]
pub struct UlogQuery {
    pub url: String
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String
}

pub async fn render_segment_ulog(
    auth: crate::middleware::auth::MyJWT,
    ViewEngine(v): ViewEngine<TeraView>, 
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    Query(params): Query<UlogQuery>
) -> Result<impl IntoResponse> {
    let request = client.get(params.url);
    // get the data and save it as a string and pass to admin_segment_ulog
    let res = request.send().await;
    let data: String;
    match res {
        Ok(response) => {
            let bytes = response.bytes().await.unwrap();
            let bytes_vec: Vec<u8> = bytes.to_vec(); // Convert &bytes::Bytes to Vec<u8>
            data = unsafe { String::from_utf8_unchecked(bytes_vec) };
        }
        _ => data = "No parsed data for this segment".to_string(),
    }
    crate::views::route::admin_segment_ulog(v, UlogText { text: data })
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectdata")
        .add("/:dongle_id/:timestamp/:segment/:file", get(file_stream))
        .add("/:filetype/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/logs/", get(render_segment_ulog))
        .add("/bootlog/:bootlog_file", get(bootlog_file_download))
}
