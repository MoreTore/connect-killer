#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State}, http::response, Extension
};
use reqwest::{StatusCode,Client};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::common;

#[derive(Deserialize)]
struct UploadUrlQuery {
    path: String,
    expiry_days: Option<i32>,
}

// Structure for handling multiple upload paths
#[derive(Deserialize)]
struct UploadUrlsQuery {
    paths: Vec<String>,  // Corrected to Vec<String> to handle multiple paths
    expiry_days: Option<i32>,
}

// Implementing the trait for expiry day validation
trait ExpiryValidation {
    fn validate_expiry(&mut self);
}

impl ExpiryValidation for UploadUrlQuery {
  fn validate_expiry(&mut self) {
    match self.expiry_days {
      Some(days) => 
        if !(days >= 1 && days <= 30) {
          self.expiry_days = Some(30);
        }
      None => {
        self.expiry_days = Some(1);
      }
    }
  }
}

impl ExpiryValidation for UploadUrlsQuery {
  fn validate_expiry(&mut self) {
    match self.expiry_days {
      Some(days) => 
        if !(days >= 1 && days <= 30) {
          self.expiry_days = Some(30);
        }
      None => {
        self.expiry_days = Some(1);
      }
    }
  }
}

#[derive(Serialize)]
struct UrlResponse {
    url: String,
}

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn get_route_files(
    //_auth: auth::JWT,
    Path(route_id): Path<String>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<Client>
) -> impl IntoResponse {

    println!("Fetching files for Route ID: {}", route_id);
    let response = get_links_for_route(ctx, &route_id, &client).await;
    match response {
        Ok((status, body)) => (status, body),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)),
    }
}

async fn get_links_for_route(ctx: AppContext, route_id: &str, client: &Client) -> Result<(StatusCode, String), Box<dyn Error>> {
    let key = common::mkv_helpers::list_keys_starting_with(&route_id.replace("|", "/")).await;
    let response = client.get(&key).send().await?;
    let code = response.status();
    let data: Value = response.json().await?;
    let keys = data["keys"].as_array().unwrap_or(&vec![]).iter()
        .map(|key| format!("{}/connectdata{}", &ctx.config.server.full_url(), key.as_str().unwrap_or_default()))
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

async fn get_upload_url(
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Query(mut params): Query<UploadUrlQuery>
) -> impl IntoResponse {
    // curl http://host/v1.4/ccfab3437bea5257/upload_url/?path=2019-06-06--11-30-31--9/fcamera.hevc&expiry_days=1
    // Assuming default expiry is 1 day if not specified
    params.validate_expiry();

    let url = format!("{}/connectincoming/{dongle_id}/{}", &ctx.config.server.full_url() ,transform_route_string(&params.path));
    Json(json!({
        //   "url": "http://host/commaincoming/239e82a1d3c855f2/2019-06-06--11-30-31/9/fcamera.hevc?sr=b&sp=c&sig=cMCrZt5fje7SDXlKcOIjHgA0wEVAol71FL6ac08Q2Iw%3D&sv=2018-03-28&se=2019-06-13T18%3A43%3A01Z"
        "url": url,
    }))
}

async fn upload_urls_handler(
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(mut data): Json<UploadUrlsQuery>,
) -> Result<Response> {
    data.validate_expiry();
    
    let urls = data.paths.iter().map(|path| {
        UrlResponse {
            url: format!("{}/connectincoming/{}/{}", &ctx.config.server.full_url(), dongle_id, transform_route_string(path)),
        }
    }).collect::<Vec<_>>();
    
    format::json(urls)
}

fn transform_route_string(input_string: &String) -> String {
// example input_string = 2024-03-02--19-02-46--0--rlog.bz2 
// converts to =          2024-03-02--19-02-46/0/rlog.bz2
let re = regex::Regex::new(r"^([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)/(.+)$").unwrap();

match re.captures(input_string) {
    Some(caps) => {
        let transformed = format!("{}--{}/{}/{}",
            &caps[1], // Date
            &caps[2], // Time
            &caps[3], // Segment number
            &caps[4]  // File name
        );
        transformed
    },
    None => "No match found".to_string(),
}
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("v1")
        .add("/route/:route_id/files", get(get_route_files))
        .add("/:dongleId/upload_urls/", post(upload_urls_handler))
        .add(".4/:dongleId/upload_url/", get(get_upload_url)) 
        .add("/echo", post(echo))
}
