#![allow(clippy::unused_async)]
use futures::StreamExt;
use loco_rs::prelude::*;
use bytes::BytesMut;
use axum::{
    extract::{Path, State},
    http::StatusCode,
  };
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    workers::{
        bootlog_parser::{
            BootlogParserWorker, 
            BootlogParserWorkerArgs
        },
        log_parser::{
            LogSegmentWorker, 
            LogSegmentWorkerArgs
        }
    },
    common,
};

pub async fn upload_bootlogs(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, file)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    body: axum::body::Body,
) -> Result<(StatusCode, &'static str)> {
    //enforce_device_upload_permission!(auth);
    let full_url = common::mkv_helpers::get_mkv_file_url(&format!("{}_boot_{}", dongle_id, file));
    
    let mut buffer = BytesMut::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => buffer.extend_from_slice(&data),
            Err(_) => return Ok((StatusCode::BAD_REQUEST, "Error reading request body")),
        }
    }
    let data = buffer.freeze();
    let data_len = data.len() as i64;
    tracing::info!("File `{}` received is {} bytes", full_url, data.len()
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
                StatusCode::FORBIDDEN => { tracing::error!("Duplicate file uploaded"); return Ok((status, "Duplicate File Upload")); }
                StatusCode::CREATED | StatusCode::OK => {
                    if let Some(device) = auth.device_model {
                        let prev_server_usage = device.server_storage;
                        let mut active_device = device.into_active_model();
                        active_device.server_storage = ActiveValue::Set(data_len + prev_server_usage);
                        match active_device.update(&ctx.db).await {
                            Ok(_) => (),
                            Err(e) => {
                                tracing::error!("Failed to update active route model. DB Error {}", e.to_string());
                            }
                        }
                
                    }
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

pub async fn upload_crash(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, id, commit, name)): Path<(String, String, String, String)>,//:dongle_id/crash/:log_id/:commit/:name
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    body: axum::body::Body,
) -> Result<(StatusCode, &'static str)> {
    //enforce_device_upload_permission!(auth);
    let full_url = common::mkv_helpers::get_mkv_file_url(&format!("{}_crash_{}_{}_{}", dongle_id, id, commit, name));
    
    let mut buffer = BytesMut::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => buffer.extend_from_slice(&data),
            Err(_) => return Ok((StatusCode::BAD_REQUEST, "Error reading request body")),
        }
    }
    let data = buffer.freeze();
    let data_len = data.len() as i64;
    tracing::info!("File `{}` received is {} bytes", full_url, data.len());

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
                StatusCode::FORBIDDEN => { tracing::error!("Duplicate file uploaded"); return Ok((status, "Duplicate File Upload")); }
                StatusCode::CREATED | StatusCode::OK => {
                    // Enqueue the file for processing
                    tracing::debug!("{full_url} file Uploaded Successfully");
                    if let Some(device) = auth.device_model {
                        let prev_server_usage = device.server_storage;
                        let mut active_device = device.into_active_model();
                        active_device.server_storage = ActiveValue::Set(data_len + prev_server_usage);
                        match active_device.update(&ctx.db).await {
                            Ok(_) => (),
                            Err(e) => {
                                tracing::error!("Failed to update active route model. DB Error {}", e.to_string());
                            }
                        }
                    }
                    return Ok((status, "File uploaded."));
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
) -> Result<(StatusCode, &'static str)> {
    //enforce_device_upload_permission!(auth);
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

    let data = buffer.freeze();
    let data_len = data.len() as i64;
    tracing::info!("File `{}` received is {} bytes", full_url, data.len());

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
                StatusCode::FORBIDDEN => { tracing::error!("Duplicate file uploaded");}
                StatusCode::CREATED | StatusCode::OK => {
                    if let Some(device) = auth.device_model {
                        let prev_server_usage = device.server_storage;
                        let mut active_device = device.into_active_model();
                        active_device.server_storage = ActiveValue::Set(data_len + prev_server_usage);
                        match active_device.update(&ctx.db).await {
                            Ok(_) => (),
                            Err(e) => {
                                tracing::error!("Failed to update active route model. DB Error {}", e.to_string());
                            }
                        }
                    }
                }
                _ => {tracing::error!("Unhandled status. File not uploaded."); return Ok((status, "Unhandled status. File not uploaded."));}
            }
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
                Ok(_) => { 
                    tracing::debug!("Queued Worker");
                    return Ok((status, "Queued Worker"));
                }
                Err(e) => {
                    tracing::error!("Failed to queue worker: {}", format!("{}", e));
                    return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue worker."));
                }
            }

        },
        Err(e) => {
            
            tracing::error!("PUT request failed: {}", format!("{}", e));
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong"));
        }
    }
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectincoming")
        .add("/:dongle_id/:timestamp/:segment/:file", put(upload_driving_logs))
        .add("/:dongle_id/crash/:log_id/:commit/:name", put(upload_crash))
        .add("/:dongle_id/boot/:file", put(upload_bootlogs))
}
