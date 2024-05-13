#![allow(clippy::unused_async)]
use loco_rs::{prelude::*};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
  
  };
use crate::common;

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}

// used for comma tools
pub async fn file_stream(
    //_auth: auth::JWT,
    Path((dongle_id, timestamp, segment, file)): Path<(String, String, String, String)>,
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap, // Include headers from the incoming request
  ) -> impl IntoResponse {
    let lookup_key = format!("{dongle_id}_{timestamp}--{segment}--{file}");
    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&lookup_key).await;
    let mut request_builder = client.get(&internal_file_url);
  
    // Check for range header and forward it if present
    if let Some(range) = headers.get(hyper::header::RANGE) {
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
    }
  
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
                response_builder = response_builder.status(StatusCode::PARTIAL_CONTENT);
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
    Path((dongle_id, timestamp, segment, file)): Path<(String, String, String, String)>,
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{dongle_id}_{timestamp}--{segment}--{file}");
    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&lookup_key).await;

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
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&bootlog_file).await;

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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectdata")
        .add("/:dongle_id/:timestamp/:segment/:file", get(file_stream))
        //https://commadata2.blob.core.windows.net/qlog/164080f7933651c4/2024-04-07--11-04-32/0/qlog.bz2
        .add("/qlog/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/rlog/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/qcam/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/fcam/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/dlog/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/elog/:dongle_id/:timestamp/:segment/:file", get(file_download))
        .add("/bootlog/:bootlog_file", get(bootlog_file_download))
        .add("/", get(hello))
        .add("/echo", post(echo))
}
