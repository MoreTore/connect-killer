#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{Path, Query, State}, Extension
};
use reqwest::{StatusCode,Client};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Write as FmtWrite;

use crate::{common, models::_entities};

use super::v1_responses::*;

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

pub async fn echo(State(ctx): State<AppContext>,
    req_body: String
) -> String {
    let ret = req_body.clone();
    crate::workers::log_parser::LogSegmentWorker::perform_later(
        &ctx,
        crate::workers::log_parser::LogSegmentWorkerArgs {
            internal_file_url: "http://localhost:3000/164080f7933651c4_2024-02-05--16-22-28--10--qlog.bz2".to_string(),
            dongle_id: "164080f7933651c4".to_string(),
            timestamp: "2024-02-05--16-22-28".to_string(),
            segment: "10".to_string(),
            file: "qlog.bz2".to_string(),
            create_time: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
            },
    ).await.unwrap();
    ret
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

async fn get_qcam_stream(
    State(ctx): State<AppContext>,
    Path(canonical_route_name): Path<String>,
) -> Result<Response> {
    let segment_models = _entities::segments::Model::find_segments_by_route(&ctx.db, &canonical_route_name).await?;
    let dongle_id = canonical_route_name.split("|").nth(0).unwrap();
    let timestamp = canonical_route_name.split("|").nth(1).unwrap();

    let mut response = String::new();
    writeln!(response, "#EXTM3U");
    writeln!(response, "#EXT-X-VERSION:3");
    writeln!(response, "#EXT-X-TARGETDURATION:61");
    writeln!(response, "#EXT-X-MEDIA-SEQUENCE:0");
    writeln!(response, "#EXT-X-PLAYLIST-TYPE:VOD");
    
    for segment in segment_models {
        writeln!(response, "#EXTINF:60.0,"); // Assuming each segment is 60 seconds long
        writeln!(response, "{}", segment.qcam_url);
    }
    
    writeln!(response, "#EXT-X-ENDLIST");
    
    Ok(response.into_response())
}

async fn get_links_for_route(
    ctx: AppContext,
    route_id: &str,
    client: &Client
) -> Result<(StatusCode, String), Box<dyn Error>> {
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
        "headers": {"Content-Type": "application/octet-stream"},
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

fn transform_route_string(input_string: &str) -> String {
    // example input_string = 2024-03-02--19-02-46--0--rlog.bz2
    // converts to =          2024-03-02--19-02-46/0/rlog.bz2
    let re_drive_log = regex::Regex::new(r"^([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)/(.+)$").unwrap();

    if let Some(caps) = re_drive_log.captures(input_string) {
        format!("{}--{}/{}/{}",
            &caps[1], // Date
            &caps[2], // Time
            &caps[3], // Segment number
            &caps[4]  // File name
        )
    } else {
        // example input_string = boot/0000008c--8a84371aea.bz2
        let re_boot_log = regex::Regex::new(r"^boot/[0-9a-z]{8}--[0-9a-z]{10}.bz2$").unwrap();
        if re_boot_log.is_match(input_string) {
            input_string.to_owned()
        } else {
            "Invalid".to_string()
        }
    }
}

async fn unpair(
    _auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let mut device_model =  _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
    device_model.owner_id = None;
    let txn = ctx.db.begin().await?;
    device_model.into_active_model().insert(&txn).await?;
    txn.commit().await?;
    format::json(r#"{"success": 1}"#)
}

async fn device_info(
    _auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    //_entities::
    format::json(DeviceInfoResponse {..Default::default()})
}

async fn device_location(
    _auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    format::json(DeviceLocationResponse {..Default::default()})
}

async fn device_stats(
    _auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    format::json(DeviceStatsResponse {..Default::default()})
}

async fn device_users(
    _auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    format::json(DeviceUsersResponse {..Default::default()})
}

#[derive(Serialize, Deserialize, Debug)]
struct DeviceSegmentQuery {
    end: i64,
    start: i64,
}

async fn route_segment(
    //_auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Query(params): Query<DeviceSegmentQuery>,
) -> Result<Response> {
    let device_model = _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
    let route_models = _entities::routes::Model::find_device_routes(&ctx.db, &dongle_id).await?;
    format::json(route_models)
}


async fn get_my_devices(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let user_model = _entities::users::Model::find_by_pid(&ctx.db, &auth.claims.pid).await?;
    let device_models = _entities::devices::Model::find_user_devices(&ctx.db, user_model.id).await;
    // let device_model = _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
    format::json(device_models)
}


// email: String,
// id: String,
// points: i64,
// regdate: i64,
// sueruser: bool,
// username: String,
async fn get_me(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let user_model = _entities::users::Model::find_by_pid(&ctx.db, &auth.claims.pid).await?;
    format::json(MeResponse { 
        email: user_model.email,
        id: String::from(user_model.pid),
        regdate: user_model.created_at.and_utc().timestamp(),
        points: user_model.points,
        superuser: user_model.superuser,
        username: user_model.name, // TODO change the usermode names to match comma api to simplify this
    })
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("v1")
        .add("/echo", post(echo))
        .add("/me", get(get_me))
        .add("/me/devices", get(get_my_devices))
        .add("/route/:route_id/files", get(get_route_files))
        .add("/route/:route_id/qcamera.m3u8", get(get_qcam_stream))
        .add("/:dongleId/upload_urls/", post(upload_urls_handler))
        .add(".4/:dongleId/upload_url/", get(get_upload_url))
        .add("/devices/:dongle_id/routes_segments", get(route_segment))
        .add("/devices/:dongle_id/unpair", post(unpair))
        .add("/devices/:dongle_id/location", get(device_location))
        .add("/devices/:dongle_id/stats", get(device_stats))
        .add("/devices/:dongle_id/users", get(device_users))
        .add(".1/devices/:dongle_id", get(device_info))
    }
