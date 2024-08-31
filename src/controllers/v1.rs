#![allow(clippy::unused_async)]
use loco_rs::{ prelude::*};
use axum::{
    extract::{Path, Query, State}, Extension
};
use reqwest::{StatusCode,Client};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::env;

use crate::{common, enforce_ownership_rule, middleware::jwt, models::{_entities}};
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

pub async fn get_route_files(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(route_id): Path<String>,
    Extension(client): Extension<Client>
) -> impl IntoResponse {
    let jwt_secret = ctx.config.get_jwt_config()?;
    if let Ok(token) = jwt::JWT::new(&jwt_secret.secret).generate_token(&(3600 * 24 as u64), auth.claims.identity.to_string()) {
        println!("Fetching files for Route ID: {}", route_id);
        let response = get_links_for_route(&route_id, &client, &token).await;
        match response {
            Ok((_status, body)) => Ok(format::json(body)),
            Err(e) => unauthorized("err"),
        }
    } else {
        return unauthorized("err");
    }

}

async fn get_qcam_stream( // TODO figure out hashing/obfuscation of the url for security
    //_auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(canonical_route_name): Path<String>,
) -> Result<Response> {

    let mut segment_models = _entities::segments::Model::find_segments_by_route(&ctx.db, &canonical_route_name).await?;
    segment_models.retain(|segment| segment.start_time_utc_millis != 0); // exclude ones wher the qlog is missing
    segment_models.sort_by(|a, b| a.number.cmp(&b.number));

    let mut response = String::new();
    response.push_str("#EXTM3U\n");
    response.push_str("#EXT-X-VERSION:3\n");
    response.push_str("#EXT-X-TARGETDURATION:61\n");
    response.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    response.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
    
    let mut prev_seg_number = match segment_models.first() {
        Some(first_seg) => first_seg.number - 1,
        None => -1, // should we throw an error instead?
    };
    for segment in segment_models {
        prev_seg_number += 1;
        if segment.number != prev_seg_number {  // Only in sequence
            break;
        }
        if segment.qcam_url != "" {
            response.push_str(&format!("#EXTINF:{},{}\n", segment.qcam_duration, segment.number));
            response.push_str(&format!("{}\n", segment.qcam_url));
        }
    }

    response.push_str("#EXT-X-ENDLIST\n");

    Ok(response.into_response())
}

async fn get_links_for_route(
    route_id: &str,
    client: &Client,
    jwt: &str
) -> Result<(StatusCode, FilesResponse), Box<dyn Error>> {
    // Assuming common::mkv_helpers::list_keys_starting_with is an async function
    let key = common::mkv_helpers::list_keys_starting_with(&route_id.replace("|", "_"));
    
    // Fetch data from the URL
    let response = client.get(&key).send().await?;
    let code = response.status();
    
    // Parse the JSON response
    let data: Value = response.json().await?;
    
    // Ensure "keys" is an array and handle potential errors
    //let keys = data.get("keys").and_then(Value::as_array).ok_or("Missing or invalid 'keys' field");
    let keys = data["keys"].as_array().unwrap();
    // Process keys to construct URLs
    let mut urls = Vec::new();
    for key in keys {
        if let Some(key_str) = key.as_str() {
            let parts: Vec<&str> = key_str.split('_').collect();
            if parts.len() == 2 {
                urls.push(format!("{}/connectdata{}/{}?sig={}",
                    env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set"),
                    parts[0],
                    transform_route_string(parts[1]),
                    jwt
                ));
            }
        }
    }
    
    // Assuming sort_keys_to_response is an async function that takes a Vec<String> and returns a FilesResponse
    let response_json = sort_keys_to_response(urls).await;
    
    Ok((code, response_json))
}


async fn sort_keys_to_response(keys: Vec<String>) -> FilesResponse {
    let mut cameras = vec![];
    let mut dcameras = vec![];
    let mut logs = vec![];
    let mut qlogs = vec![];
    let mut qcameras = vec![];
    let mut ecameras = vec![];

    for key in keys {
        if key.contains("fcamera.hevc") {
            cameras.push(key.into());
            continue;
        } else if key.contains("dcamera.hevc") {
            dcameras.push(key);
            continue;
        } else if key.contains("qcamera.ts") {
            qcameras.push(key.into());
            continue;
        } else if key.contains("ecameras.hevc") {
            ecameras.push(key.into());
            continue;
        } else if key.contains("qlog") {
            qlogs.push(key.into());
            continue;
        } else if key.contains("rlog") {
            logs.push(key.into());
            continue;
        }

    }
    FilesResponse {
        cameras: cameras,
        dcameras: dcameras,
        logs: logs,
        qlogs: qlogs,
        qcameras: qcameras,
        ecameras: ecameras,
    }
}

async fn get_upload_url(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Query(mut params): Query<UploadUrlQuery>
) -> impl IntoResponse {
    if let Some(device_model) = auth.device_model {
        if !device_model.uploads_allowed {
            return unauthorized("Uploads ignored");
        }
    } else {
        return unauthorized("Only registered devices can upload");
    }
    let upload_url = format!("{}/connectincoming/{dongle_id}/{}",
        env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set"),
        transform_route_string(&params.path));
    tracing::info!("Device will upload to {upload_url}");
    // curl http://host/v1.4/ccfab3437bea5257/upload_url/?path=2019-06-06--11-30-31--9/fcamera.hevc&expiry_days=1
    // Assuming default expiry is 1 day if not specified
    params.validate_expiry();
    let jwt_secret = ctx.config.get_jwt_config()?;
    if let Ok(token) = jwt::JWT::new(&jwt_secret.secret)
        .generate_token(
            &(3600 * 24 as u64), 
            auth.claims.identity.to_string()) {
        Ok(Json(json!({
            //   "url": "http://host/commaincoming/239e82a1d3c855f2/2019-06-06--11-30-31/9/fcamera.hevc?sr=b&sp=c&sig=cMCrZt5fje7SDXlKcOIjHgA0wEVAol71FL6ac08Q2Iw%3D&sv=2018-03-28&se=2019-06-13T18%3A43%3A01Z"
            "url": upload_url,
            "headers": {"Content-Type": "application/octet-stream",
                        "Authorization": format!("JWT {}", token)},
        })))
    } else {
        return loco_rs::controller::bad_request("failed to generate token")
    }
}

async fn upload_urls_handler(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(mut data): Json<UploadUrlsQuery>,
) -> Result<Response> {
    data.validate_expiry();
    let jwt_secret = ctx.config.get_jwt_config()?;
    if let Ok(token) = jwt::JWT::new(&jwt_secret.secret)
    .generate_token(
        &(3600 * 24 as u64), 
        auth.claims.identity.to_string()) {
        let urls = data.paths.iter().map(|path| {
            UrlResponse {
                url: format!("{}/connectincoming/{dongle_id}/{}?sig={token}",
                    env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set"),
                    transform_route_string(path),
                ),
            }
        }).collect::<Vec<_>>();
        return format::json(urls)
    } else {
        return loco_rs::controller::bad_request("failed to generate token")
    }
}

fn transform_route_string(input_string: &str) -> String {
    use crate::common::re::*;
    let segment_file_regex_string = format!(
        r"^({ROUTE_NAME})--({NUMBER})(?:--|/)({ALLOWED_FILENAME}$)"
    );
    let boot_file_regex_string = format!(
        r"^boot/({ROUTE_NAME}).bz2$"
    );
    let crash_file_regex_string = format!(
        r"^crash/({ROUTE_NAME})_([0-9a-f]{{8}})_(.+)$"
    );
    // example input_string = 2024-03-02--19-02-46--0--rlog.bz2 or 2024-03-02--19-02-46--0/rlog
    // converts to =          2024-03-02--19-02-46/0/rlog.bz2
    let re_drive_log = regex::Regex::new(&segment_file_regex_string).unwrap();
    // or for openpilot version 0.9.7+ the new format is 0000008c--8a84371aea--0/rlog.bz2
    // the crash log format is crash/0000008c--8a84371aea_<8 digit hex serial>__<crash name>
    let re_crash_log = regex::Regex::new(&crash_file_regex_string).unwrap();
    let re_boot_log = regex::Regex::new(&boot_file_regex_string).unwrap();
    if let Some(caps) = re_drive_log.captures(input_string) {
        format!("{}/{}/{}",
            &caps[1], // DateTime or monotonic--uid
            &caps[2], // Segment number
            &caps[3]  // File name
        )
    } else if let Some(caps) = re_crash_log.captures(input_string) {
        format!("crash/{}/{}/{}",
            &caps[1], // ID
            &caps[2], // commit
            &caps[3] // name
        )
    } else if re_boot_log.is_match(input_string) {
        input_string.to_owned()
    } else {
        "Invalid".to_string()
    }
}

async fn unpair(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let user_model = _entities::users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
    let device_model =  _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
    if !user_model.superuser {
        enforce_ownership_rule!(
            user_model.id, 
            device_model.owner_id, 
            "Can only unpair your own device!"
        );
    }
    let mut active_device_model = device_model.into_active_model();
    active_device_model.owner_id = ActiveValue::Set(None);
    active_device_model.update(&ctx.db).await?;
    format::json(r#"{"success": 1}"#)
}

async fn device_info(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let device = match auth.device_model {
        Some(device) => device,
        None => _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?,
    };
    format::json(
        DeviceInfoResponse {
            dongle_id: device.dongle_id,
            alias: device.alias,
            serial: device.serial,
            //athena_host: device.
            last_athena_ping: device.last_athena_ping,
            ignore_uploads: !device.uploads_allowed,
            is_paired: device.owner_id.is_some(),
            //is_owner: device.
            public_key: device.public_key,
            prime: device.prime,
            prime_type: device.prime_type,
            trial_claimed: true,
            //device_type: device.
            //last_gps_time: device.
            //last_gps_lat: device.
            //last_gps_lng: device.
            //last_gps_accur: device.
            //last_gps_speed: device.
            //last_gps_bearing: device.
            //openpilot_version: device.
            sim_id: device.sim_id,

            ..Default::default()
        }
    )
}

async fn device_location(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let user_model = _entities::users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
    let device_model =  _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
    if !user_model.superuser {
        enforce_ownership_rule!(
            user_model.id, 
            device_model.owner_id, 
            "Can only locate owned devices!"
        );
    }
    // get most recent route with gps
    let (lat, lng, time) = _entities::routes::Model::find_latest_pos(&ctx.db, &dongle_id).await?;
    let response = DeviceLocationResponse {
        dongle_id,
        lat,
        lng,
        time,
        ..Default::default()
    };

    format::json(response)
}

async fn device_stats(
    _auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    use std::time::{SystemTime, UNIX_EPOCH, Duration};

    // Get the current time in milliseconds since the UNIX epoch
    let utc_time_now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Calculate the start of the week (7 days ago)
    let one_week_ago_millis = utc_time_now_millis - Duration::from_secs(7 * 24 * 60 * 60).as_millis() as i64;

    // Get total stats
    let (total_length, route_count) = _entities::routes::Model::total_length_and_count_time_filtered(
        &ctx.db,
        &dongle_id,
        None, // No time filter for total stats
        None,
    ).await?;

    // Get stats for the past week
    let (week_length, week_count) = _entities::routes::Model::total_length_and_count_time_filtered(
        &ctx.db,
        &dongle_id,
        Some(one_week_ago_millis), // From one week ago
        Some(utc_time_now_millis), // To now
    ).await?;

    let ret = DeviceStatsResponse{
        all: DeviceStats {
            distance: total_length,
            routes: route_count,
            ..Default::default()
        },
        week: DeviceStats {
            distance: week_length,
            routes: week_count,
            ..Default::default()
        },
    };

    format::json(ret)
}

async fn device_users(
    _auth: crate::middleware::auth::MyJWT,
    State(_ctx): State<AppContext>,
    Path(_dongle_id): Path<String>,
) -> Result<Response> {
    format::json(DeviceUsersResponse {..Default::default()})
}

#[derive(Serialize, Deserialize, Debug)]
struct DeviceSegmentQuery {
    end: Option<i64>,
    start: Option<i64>,
    limit: Option<u64>,
}

async fn route_segment(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Query(params): Query<DeviceSegmentQuery>,
) -> Result<Response> {
    if let Some(user_model) = auth.user_model {
        if user_model.superuser {

        } else {
            let _ = _entities::devices::Model::find_user_device(&ctx.db, user_model.id, &dongle_id).await?; // just error if not found
        }
    }
    let mut route_models = _entities::routes::Model::find_time_filtered_device_routes(&ctx.db, &dongle_id, params.start, params.end, params.limit).await?;
    route_models.retain(|route| route.maxqlog != -1); // exclude ones wher the qlog is missing
    format::json(route_models)
}

async fn route_info(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(fullname): Path<String>,
) -> Result<Response> {
    let route_model = _entities::routes::Model::find_route(&ctx.db, &fullname).await?;
    if let Some(user_model) = auth.user_model {
        if user_model.superuser {

        } else {
            let _ = _entities::devices::Model::find_user_device(&ctx.db, user_model.id, &route_model.device_dongle_id).await?; // just error if not found
        }
    }
    format::json(route_model)
}


async fn preserved_routes( // TODO
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    if let Some(user_model) = auth.user_model {
        if user_model.superuser {

        } else {
            let _ = _entities::devices::Model::find_user_device(&ctx.db, user_model.id, &dongle_id).await?; // just error if not found
        }
    }
    let route_models = _entities::routes::Model::find_device_routes(&ctx.db, &dongle_id).await?;
    format::json(route_models)
}


async fn get_my_devices(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    // TODO: implement authorized devices!
    let user_model = _entities::users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
    let device_models = if user_model.superuser {
        _entities::devices::Model::find_all_devices(&ctx.db).await
    } else {
        _entities::devices::Model::find_user_devices(&ctx.db, user_model.id).await
    };
    let mut devices = vec![];
    for device_model in device_models {
        let device = DeviceResponse {
            alias: device_model.alias,
            //athena_host: String,
            device_type: device_model.device_type,
            dongle_id: device_model.dongle_id,
            ignore_uploads: !device_model.uploads_allowed, // flip this
            is_owner: (device_model.owner_id == Some(user_model.id)),
            is_paired: device_model.owner_id.is_some(),
            last_athena_ping: device_model.last_athena_ping,
            //last_gps_accuracy: device.
            //last_gps_bearing: device.
            //last_gps_lat: 0.0
            //last_gps_lng: 0.0, //todo
            //last_gps_speed: 0, // todo
            //last_gps_time: device.last_athena_ping, //  Todo
            //openpilot_version: device_model.openpilot_version,
            prime: true,
            prime_type: 4,
            public_key: device_model.public_key,
            serial: device_model.serial,
            sim_id: device_model.sim_id,
            trial_claimed: true,
            online: device_model.online,
            ..Default::default()

        };
        devices.push(device);
    }

    format::json(devices)
}


async fn get_me(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_model = _entities::users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
    format::json(MeResponse {
       email: user_model.email,
       id: String::from(user_model.identity),
       regdate: user_model.created_at.and_utc().timestamp(),
       points: user_model.points,
       superuser: user_model.superuser,
       username: user_model.name, // TODO change the usermode names to match comma api to simplify this
    })
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
struct Destination {
    latitude: f64,
    longitude: f64,
    place_details: String,
    place_name: String,
}

async fn set_destination(
    auth: crate::middleware::auth::MyJWT,   
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(destination): Json<Destination>,
    //axum::Extension(connection_manager): axum::Extension<std::sync::Arc<crate::controllers::ws::ConnectionManager>>
) -> impl IntoResponse {
    use crate::controllers::ws::JsonRpcRequest;

    let mut active_device;
    let mut is_online = false;
    if let Some(device_model) = auth.device_model {
        is_online = device_model.online;
        active_device = device_model.into_active_model();
    } else if let Some(user_model) = auth.user_model {
        let device_model = _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
        is_online = device_model.online;
        enforce_ownership_rule!(
            user_model.id, 
            device_model.owner_id, 
            "Can only edit owned devices locations!"
        ); // early return if not owned
        active_device = device_model.into_active_model();
    } else {
        return Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response());
    }

    let msg = JsonRpcRequest{
        method: "setNavDestination".to_string(),
        params: Some(serde_json::to_value(destination.clone())?),
        ..Default::default()
    };
    _entities::device_msg_queues::Model::insert_msg(&ctx.db, &dongle_id, msg).await?;

    // Deserialize the current locations
    let mut locations: Vec<SavedLocation> = if let Some(locations_json) = active_device.locations.as_ref() {
        serde_json::from_value(locations_json.clone()).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Check if the label exists and update, otherwise add the new location
    let mut location_found = false;
    for loc in locations.iter_mut() {
        if loc.label == Some(destination.place_name.clone()) {
            // Update existing location
            loc.place_name = destination.place_name.clone();
            loc.place_details = destination.place_details.clone();
            loc.latitude = destination.latitude;
            loc.longitude = destination.longitude;
            //loc.save_type = "recent".to_string();
            loc.modified = chrono::Utc::now().timestamp_millis().to_string();
            loc.next = !is_online;
            location_found = true;
            break;
        }
    }

    if !location_found {
        // Create a new SavedLocation entry
        let new_location = SavedLocation {
            id: uuid::Uuid::new_v4(),
            dongle_id: dongle_id.clone(),
            place_name: destination.place_name.clone(),
            place_details: destination.place_details,
            latitude: destination.latitude,
            longitude: destination.longitude,
            save_type: "recent".to_string(),
            label: Some(destination.place_name),
            modified: chrono::Utc::now().timestamp_millis().to_string(),
            next: !is_online,
        };
        locations.push(new_location);
    }
    // Serialize the updated locations back to JSON
    active_device.locations = ActiveValue::Set(Some(serde_json::to_value(locations)?));

    let mut response;
    // Update the device model in the database
    match active_device.update(&ctx.db).await {
        Ok(_) => {
            response = serde_json::json!({
                "success": true,
                "dongle_id": dongle_id,
                "saved_next": !is_online
            });
        }, // Respond with success
        Err(e) => {
            tracing::error!("Failed to update device locations. DB Error {}", e);
            response = serde_json::json!({
                //"success": false,
                "error": true,
                //"dongle_id": dongle_id,
                //"saved_next": false
            });
        }
    }

    format::json(response)

}


async fn get_next_destination(
    auth: crate::middleware::auth::MyJWT,   
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> impl IntoResponse {
    if let Some(mut device_model) = auth.device_model {
        // Deserialize the current locations from the device model
        if let Some(locations_json) = device_model.locations.as_ref() {
            let mut locations: Vec<SavedLocation> =
                serde_json::from_value(locations_json.clone()).unwrap_or_default();

            // Find the next location and clone it
            if let Some(next_location) = locations.iter_mut().find(|loc| loc.next) {
                let cloned_location = next_location.clone();
                // Clear the next flag in the location
                next_location.next = false;

                // Convert the device model to an active model
                let mut active_device_model = device_model.into_active_model();

                // Update the device model with the modified locations
                active_device_model.locations = ActiveValue::Set(Some(serde_json::to_value(&locations).unwrap()));

                // Save the updated device model
                if let Err(e) = active_device_model.update(&ctx.db).await {
                    tracing::error!("Failed to update device locations. DB Error: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, format::json(serde_json::Value::Null)).into_response();
                }

                // Return the next location as the response
                return Json(json!({
                    "place_name": cloned_location.place_name,
                    "place_details": cloned_location.place_details,
                    "latitude": cloned_location.latitude,
                    "longitude": cloned_location.longitude
                }))
                .into_response();
            }
        }
    }

    // Return null if no next location is found
    format::json(serde_json::Value::Null).into_response()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SavedLocation {
    next: bool,
    id: uuid::Uuid,
    dongle_id: String,
    place_name: String,
    place_details: String,
    latitude: f64,
    longitude: f64,
    save_type: String,  // Could be an enum, but using String for simplicity
    label: Option<String>,
    modified: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PutSavedLocation {
    place_name: String,
    place_details: String,
    latitude: f64,
    longitude: f64,
    save_type: String,  // Could be an enum, but using String for simplicity
    label: Option<String>,
}

async fn put_locations(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(destination): Json<PutSavedLocation>,
) -> Result<Response, loco_rs::Error> {
    let mut active_device;

    if let Some(device_model) = auth.device_model {
        active_device = device_model.into_active_model();
    } else if let Some(user_model) = auth.user_model {
        let device_model = _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
        enforce_ownership_rule!(
            user_model.id, 
            device_model.owner_id, 
            "Can only edit owned devices locations!"
        ); // early return if not owned
        active_device = device_model.into_active_model();
    } else {
        return Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response());
    }

    // Deserialize the current locations
    let mut locations: Vec<SavedLocation> = if let Some(locations_json) = active_device.locations.as_ref() {
        serde_json::from_value(locations_json.clone()).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Check if the label exists and update, otherwise add the new location
    let mut location_found = false;
    for loc in locations.iter_mut() {
        if loc.label == destination.label {
            // Update existing location
            loc.place_name = destination.place_name.clone();
            loc.place_details = destination.place_details.clone();
            loc.latitude = destination.latitude;
            loc.longitude = destination.longitude;
            loc.save_type = destination.save_type.clone();
            loc.modified = chrono::Utc::now().timestamp_millis().to_string();
            location_found = true;
            break;
        }
    }

    if !location_found {
        // Create a new SavedLocation entry
        let new_location = SavedLocation {
            id: uuid::Uuid::new_v4(),
            dongle_id: dongle_id.clone(),
            place_name: destination.place_name.clone(),
            place_details: destination.place_details.clone(),
            latitude: destination.latitude,
            longitude: destination.longitude,
            save_type: destination.save_type.clone(),
            label: if destination.label.is_none() {
                Some(destination.place_name.clone())
            } else {
                destination.label.clone()
            },            
            modified: chrono::Utc::now().timestamp_millis().to_string(),
            next: false,
        };
        locations.push(new_location);
    }

    // Serialize the updated locations back to JSON
    active_device.locations = ActiveValue::Set(Some(serde_json::to_value(locations)?));

    // Update the device model in the database
    match active_device.update(&ctx.db).await {
        Ok(_) => Ok((StatusCode::OK, Json(json!({ "success": true }))).into_response()), // Respond with success
        Err(e) => {
            tracing::error!("Failed to update device locations. DB Error {}", e);
            Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Failed to update location" }))).into_response())
        }
    }
}

async fn get_locations(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    //axum::Extension(shared_state): axum::Extension<reqwest::Client>,
) -> Result<Response> {
    if let Some(device_model) = auth.device_model {
        let locations = device_model.locations.unwrap_or_default();
        return Ok((StatusCode::OK, Json(locations)).into_response());
    } else if let Some(user_model) = auth.user_model {
        let device_model =  _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
        if !user_model.superuser {
            enforce_ownership_rule!(
                user_model.id, 
                device_model.owner_id, 
                "Can only see owned devices location!"
            ); // early return if not owned
        }
        let locations = device_model.locations.unwrap_or_default();
        return Ok((StatusCode::OK, Json(locations)).into_response());
    } else {
        return Ok((StatusCode::NO_CONTENT, format::json("")).into_response());
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct DeleteLocation {
    id: String,
}

async fn delete_location(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(payload): Json<DeleteLocation>,
) -> Result<Response, loco_rs::Error> {
    let mut active_device;

    if let Some(device_model) = auth.device_model {
        active_device = device_model.into_active_model();
    } else if let Some(user_model) = auth.user_model {
        let device_model = _entities::devices::Model::find_device(&ctx.db, &dongle_id).await?;
        if !user_model.superuser {
            enforce_ownership_rule!(
                user_model.id, 
                device_model.owner_id, 
                "Can only edit owned devices locations!"
            ); // early return if not owned
        }
        active_device = device_model.into_active_model();
    } else {
        return Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response());
    }

    // Deserialize the current locations
    let mut locations: Vec<SavedLocation> = if let Some(locations_json) = active_device.locations.as_ref() {
        serde_json::from_value(locations_json.clone()).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Find and remove the location with the matching id
    let original_len = locations.len();
    locations.retain(|loc| loc.id.to_string() != payload.id);

    if locations.len() == original_len {
        // If no location was removed, respond with an error
        return Ok((StatusCode::NOT_FOUND, Json(json!({ "error": "Location not found" }))).into_response());
    }

    // Serialize the updated locations back to JSON
    active_device.locations = ActiveValue::Set(Some(serde_json::to_value(locations)?));

    // Update the device model in the database
    match active_device.update(&ctx.db).await {
        Ok(_) => Ok((StatusCode::OK, Json(json!({ "success": true }))).into_response()),
        Err(e) => {
            tracing::error!("Failed to update device locations after deletion. DB Error {}", e);
            Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Failed to delete location" }))).into_response())
        }
    }
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("v1")
        //.add("/echo", post(echo))
        .add("/me", get(get_me))
        .add("/me/devices", get(get_my_devices))
        .add("/route/:fullname", get(route_info))
        .add("/route/:fullname/files", get(get_route_files))
        .add("/route/:fullname/qcamera.m3u8", get(get_qcam_stream))
        .add("/:dongleId/upload_urls/", post(upload_urls_handler))
        .add(".4/:dongleId/upload_url/", get(get_upload_url))
        .add("/devices/:dongle_id/routes_segments", get(route_segment))
        .add("/devices/:dongle_id/routes/preserved", get(preserved_routes))
        .add("/devices/:dongle_id/unpair", post(unpair))
        .add("/devices/:dongle_id/location", get(device_location))
        .add(".1/devices/:dongle_id/stats", get(device_stats))
        .add("/devices/:dongle_id/users", get(device_users))
        .add(".1/devices/:dongle_id", get(device_info))
        .add("/navigation/:dongle_id/set_destination", post(set_destination))
        .add("/navigation/:dongle_id/locations", get(get_locations).put(put_locations).delete(delete_location))
        .add("/navigation/:dongle_id/next", get(get_next_destination))
    }
