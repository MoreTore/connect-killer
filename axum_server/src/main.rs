use axum::{
  body::Bytes,
  extract::{Path, State, Query},
  http::{HeaderMap, StatusCode},
  response::IntoResponse,
  routing::get,
  Router,

};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use std::error::Error;
use tower_http::trace::TraceLayer;
use tracing::{debug, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use serde::Deserialize;
#[derive(Deserialize)]
struct UploadUrlQuery {
    path: String,
    expiry_days: Option<i32>,
}


const MKV_ENDPOINT: &str = "http://localhost:3000";

async fn list_keys_starting_with(str: &str) -> String {
  format!("{}/{}?list", MKV_ENDPOINT, str)
}


#[tokio::main]
async fn main() {
  tracing_subscriber::registry()
      .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "example_reqwest_response=debug,tower_http=debug".into()))
      .with(tracing_subscriber::fmt::layer())
      .init();

  let client = Client::new();
  let app = Router::new()
      //.route("/", get(proxy_via_reqwest))
      .route("/v1/route/:route_id/files", get(get_route_files))
      .route("/v1.4/:dongleId/upload_url", get(get_upload_url)) // curl https://api.commadotai.com/v1.4/ccfab3437bea5257/upload_url/?path=2019-06-06--11-30-31--9/fcamera.hevc&expiry_days=1 -H 'Authorization: JWT jwt_signed_with_device_private_key'
      .route("/v1/:dongleId/upload_urls", get(get_upload_urls))
      .layer(TraceLayer::new_for_http().on_body_chunk(
          |chunk: &Bytes, _latency: Duration, _span: &Span| {
              debug!("Streaming {} bytes", chunk.len());
          },
      ))
      .with_state(client);

  let listener = tokio::net::TcpListener::bind("127.0.0.1:6734")
  .await
  .unwrap();
tracing::debug!("listening on {}", listener.local_addr().unwrap());
axum::serve(listener, app).await.unwrap();
}

async fn get_route_files(
  Path(route_id): Path<String>,
  State(client): State<Client>,
  headers: HeaderMap,
) -> impl IntoResponse {
  debug!("Fetching files for Route ID: {}", route_id);
  let response = get_links_for_route(&route_id, &client).await;
  match response {
      Ok((status, body)) => (status, body),
      Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)),
  }
}

async fn get_upload_url(
  Path(dongle_id): Path<String>,
  Query(params): Query<UploadUrlQuery>
) -> impl IntoResponse {
  // Assuming default expiry is 1 day if not specified
  let expiry = params.expiry_days.unwrap_or(1);
  json!({
      "dongle_id": dongle_id,
      "path": params.path,
      "expiry_days": expiry,
      "url": format!("https://api.example.com/upload/{}?expiry={}", params.path, expiry)
  }).to_string()
}

async fn get_upload_urls(
  Path(dongle_id): Path<String>,
  Query(params): Query<UploadUrlQuery>
) -> impl IntoResponse {
  // Assuming default expiry is 1 day if not specified
  let expiry = params.expiry_days.unwrap_or(1);

  json!({
      "dongle_id": dongle_id,
      "path": params.path,
      "expiry_days": expiry,
      "url": format!("https://api.example.com/upload/{}?expiry={}", params.path, expiry)
  }).to_string()
}

async fn get_links_for_route(route_id: &str, client: &Client) -> Result<(StatusCode, String), Box<dyn Error>> {
  let key = list_keys_starting_with(&route_id.replace("|", "_")).await;
  let response = client.get(&key).send().await?;
  let code = response.status();
  let data: Value = response.json().await?;
  let keys = data["keys"].as_array().unwrap_or(&vec![]).iter()
      .map(|key| format!("{}{}", MKV_ENDPOINT, key.as_str().unwrap_or_default()))
      .collect::<Vec<String>>();
  let response_json = sort_keys_to_response(keys).await;

  Ok((code, response_json.to_string()))
}

async fn sort_keys_to_response(keys: Vec<String>) -> Value {
  let mut response_json = json!({
    "cameras": [],
    "dcameras": [],
    "logs": [],
    "qlogs": [],
    "qcameras": []
  });
  for key in keys {
    if key.contains("rlog") && !key.contains("qlog") {
      response_json["logs"].as_array_mut().unwrap().push(key.into());
    } else if key.contains("fcamera.hevc") {
      response_json["cameras"].as_array_mut().unwrap().push(key.into());
    } else if key.contains("dcamera.hevc") {
      response_json["dcameras"].as_array_mut().unwrap().push(key.into());
    } else if key.contains("qcamera.hvec") {
      response_json["qcameras"].as_array_mut().unwrap().push(key.into());
    } else if key.contains("qlogs") && key.contains("rlog") {
      response_json["qlogs"].as_array_mut().unwrap().push(key.into());
    }
  }
  response_json
}