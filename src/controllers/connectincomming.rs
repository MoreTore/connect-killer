#![allow(clippy::unused_async)]
use futures::{StreamExt};
use loco_rs::prelude::*;
use bytes::BytesMut;
use crate::enforce_device_upload_permission;
use crate::common;
use crate::workers::bootlog_parser::{BootlogParserWorker, BootlogParserWorkerArgs};
use crate::workers::log_parser::{LogSegmentWorker, LogSegmentWorkerArgs};
use axum::{
    extract::{Path, State},
    http::{StatusCode},
    response::{IntoResponse, Response},
    routing::get,
  
  };
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}

pub async fn upload_bootlogs(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, file)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    body: axum::body::Body,
) -> impl IntoResponse {
    enforce_device_upload_permission!(auth);
    let full_url = common::mkv_helpers::get_mkv_file_url(&format!("{}_{}", dongle_id, file));
    
    let mut buffer = BytesMut::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => buffer.extend_from_slice(&data),
            Err(_) => return Ok((StatusCode::BAD_REQUEST, "Error reading request body")),
        }
    }
    let data = buffer.freeze();
    tracing::info!(
        "File `{}` received is {} bytes",
        full_url, data.len()
    );
    // Post the binary data to the specified URL
    let response = client.put(&full_url)
        .body(data)
        .send()
        .await;
    match response {
        Ok(response) => {
            let status = response.status();
            tracing::trace!("Got Ok response with status {status}");
            match status {
                StatusCode::FORBIDDEN => { tracing::trace!("Duplicate file uploaded"); return Ok((status, "Duplicate File Upload")); }
                StatusCode::CREATED | StatusCode::OK => {
                    // Enqueue the file for processing
                    tracing::debug!("File Uploaded Successfully. Queuing worker for {full_url}");
                    let result = BootlogParserWorker::perform_later(&ctx, 
                    BootlogParserWorkerArgs {
                        internal_file_url: full_url.clone(),
                        dongle_id: dongle_id,
                        file_name: file,
                        create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                    },
                    ).await;
                    match result { // sorry this is kinda confusing
                        Ok(_) => { tracing::debug!("Queued Worker"); return Ok((status, "Queued Worker"));}
                        Err(e) => {
                            tracing::error!("Failed to queue worker: {}", format!("{}", e));
                            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue worker."));
                        }
                    }
                }
                _ => {tracing::error!("Unhandled status. File not uploaded."); return Ok((status, "Unhandled status. File not uploaded."));}
            }
        },
        Err(e) => {
            
            tracing::error!("PUT request failed: {}", format!("{}", e));
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong"));
        }
    }
}


pub async fn upload_driving_logs(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, timestamp, segment, file)): Path<(String, String, String, String)>,
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    body: axum::body::Body,
) -> impl IntoResponse {
    enforce_device_upload_permission!(auth);
    // Construct the URL to store the file
    let full_url = common::mkv_helpers::get_mkv_file_url(&format!("{}_{}--{}--{}", dongle_id, timestamp, segment, file));
    tracing::trace!("full_url: {full_url}");
    // Check for duplicate file
    //let response = client.request(&full_url).send().await;

    // Collect the binary data from the body
    let mut buffer = BytesMut::new();
    let mut stream = body.into_data_stream();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => buffer.extend_from_slice(&data),
            Err(_) => return Ok((StatusCode::BAD_REQUEST, "Error reading request body")),
        }
    }

    println!(
        "File `{}` received from `{}` is {} bytes",
        file, dongle_id, buffer.len()
    );

    let data = buffer.freeze();


    // Post the binary data to the specified URL
    let response = client.put(&full_url)
        .body(data)
        .send()
        .await;

    match response {
        Ok(response) => {
            let status = response.status();
            tracing::trace!("Got Ok response with status {status}");
            match status {
                StatusCode::FORBIDDEN => { tracing::trace!("Duplicate file uploaded"); return Ok((status, "Duplicate File Upload")); }
                StatusCode::CREATED | StatusCode::OK => {
                    // Enqueue the file for processing
                    tracing::debug!("File Uploaded Successfully. Queuing worker for {full_url}");
                    let result = LogSegmentWorker::perform_later(&ctx, 
                        LogSegmentWorkerArgs {
                            internal_file_url: full_url,
                            dongle_id: dongle_id,
                            timestamp: timestamp,
                            segment: segment,
                            file: file,
                            create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                        },
                    ).await;
                    match result {
                        Ok(_) => { tracing::debug!("Queued Worker"); return Ok((status, "Queued Worker"));}
                        Err(e) => {
                            tracing::error!("Failed to queue worker: {}", format!("{}", e));
                            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue worker."));
                        }
                    }
                }
                _ => {tracing::error!("Unhandled status. File not uploaded."); return Ok((status, "Unhandled status. File not uploaded."));}
            }
        },
        Err(e) => {
            
            tracing::error!("PUT request failed: {}", format!("{}", e));
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong"));
        }
    }
    return unauthorized("error"); 
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectincoming")
        .add("/:dongle_id/:timestamp/:segment/:file", put(upload_driving_logs))
        .add("/:dongle_id/boot/:file", put(upload_bootlogs))
        .add("/", get(hello))
        .add("/echo", post(echo))
}
