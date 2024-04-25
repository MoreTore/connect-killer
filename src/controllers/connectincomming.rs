#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use bytes::BytesMut;
use crate::common;
use crate::workers::qlog_parser::{QlogParserWorker, QlogParserWorkerArgs};
use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
  
  };
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}

pub async fn upload_to_mkv_server(
    Path((dongle_id, timestamp, segment, file)): Path<(String, String, String, String)>,
    State(ctx): State<AppContext>,
    axum::Extension(client): axum::Extension<reqwest::Client>,
    mut multipart: Multipart,
  ) -> impl IntoResponse {
  while let Ok(Some(mut field)) = multipart.next_field().await {
    let name = field.name().unwrap().to_string();
    let file_name = field.file_name().unwrap().to_string();
    let full_url = common::mkv_helpers::get_mkv_file_url(&file).await;

    let mut buffer = BytesMut::new();
    while let Some(chunk) = field.chunk().await.unwrap_or_else(|_| Default::default()) {
      buffer.extend_from_slice(&chunk);
    }

    println!(
      "Length of `{}` (`{}`) is {} bytes",
      name, file_name, buffer.len()
    );

    let data = buffer.freeze();

    let response = client.put(&full_url)
      .body(data)
      .send()
      .await;

    match response {
      Ok(response) => {
        let status = response.status();
        match status {
          StatusCode::FORBIDDEN => return (status, "Duplicate File Upload"),
          StatusCode::CREATED => {
            // enque the file for processing
            let result = QlogParserWorker::perform_later(&ctx, 
              QlogParserWorkerArgs {
                internal_file_url: full_url,
                dongle_id: dongle_id,
                timestamp: timestamp,
                segment: segment,
                file: file,
                create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
              },
            ).await;
            match result {
              Ok(_) => return (status, "File Uploaded Successfully"),
              Err(e) => {
                let error_message = format!("{}", e);
                println!("Error enqueuing file for processing: {}", error_message);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong");
              }
            }
          }
           
          _ => return (status, "Unhandled status. File not uploaded.")
        }
      },
      Err(e) => {
        let error_message = format!("{}", e);
        println!("PUT request failed: {}", error_message);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong");
      }
    }
  }

  (StatusCode::BAD_REQUEST, "Invalid multipart file uplaod request")
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectincomming")
        .add("/:dongle_id/:timestamp/:segment/:file", post(upload_to_mkv_server))
        .add("/", get(hello))
        .add("/echo", post(echo))
}
