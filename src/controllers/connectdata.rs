#![allow(clippy::unused_async)]
use loco_rs::{controller, prelude::*};
use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
  
  };
use crate::common;

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}


pub async fn file_stream(
    _auth: auth::JWT,
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectdata")
        .add("/:dongle_id/:timestamp/:segment/:file", get(file_stream))
        .add("/", get(hello))
        .add("/echo", post(echo))
}
