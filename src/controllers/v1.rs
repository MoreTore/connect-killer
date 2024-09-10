
use loco_rs::prelude::*;
use axum::{
    extract::{Path, Query, State}, routing::patch, Extension
};
use reqwest::{StatusCode,Client};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::{
    env, 
    time::{SystemTime,
        UNIX_EPOCH,
        Duration
    },
    error::Error
};

use crate::{common, 
    middleware::jwt, 
    models::{
        devices::DM,
        segments::SM,
        routes::RM,
        users::UM,
        device_msg_queues::DMQM,
    }
};
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
    // Do not need to check for data ownership because its done when you try to fetch the data
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

async fn get_qcam_stream(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(canonical_route_name): Path<String>,
) -> Result<Response> {
    // Do not need to check for data ownership because its done when you try to fetch the data
    let mut segment_models = SM::find_segments_by_route(&ctx.db, &canonical_route_name).await?;
    segment_models.retain(|segment| segment.start_time_utc_millis != 0); // exclude ones wher the qlog is missing
    segment_models.sort_by(|a, b| a.number.cmp(&b.number));

    let exp = (3600 * 24 as u64);
    let jwt_secret = ctx.config.get_jwt_config()?;
    let token = jwt::JWT::new(&jwt_secret.secret)
        .generate_token(
        &exp,
        auth.claims.identity.to_string()).unwrap_or_default();
    
    for seg in segment_models.iter_mut() {
        seg.qcam_url = format!("{}?exp={}&sig={}",seg.qcam_url, exp, token)
    }

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
    let key = common::mkv_helpers::list_keys_starting_with(&route_id.replace("|", "_"));
    // Fetch data from the URL
    let response = client.get(&key).send().await?;
    let code = response.status();
    // Parse the JSON response
    let data: Value = response.json().await?;
    // Ensure "keys" is an array and handle potential errors
    let keys = data["keys"].as_array();
    // Process keys to construct URLs
    let mut urls = Vec::new();

    if let Some(keys) = keys {
        keys.iter().filter_map(|key| key.as_str()).for_each(|key_str| {
            if let [prefix, route] = key_str.split('_').collect::<Vec<_>>()[..] {
                urls.push(format!("{}/connectdata{}/{}?sig={}",
                    env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set"),
                    prefix,
                    transform_route_string(route),
                    jwt
                ));
            }
        });
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
    let device_model = if let Some(device_model) = auth.device_model{
        device_model
    } else if let Some(user_model) = auth.user_model {
        if user_model.superuser {
            DM::find_device(&ctx.db, &dongle_id)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "no device model"))?
        } else {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "no device model"))?
        }
    } else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "no device or user model"));
    };    
    
    if !device_model.uploads_allowed {
        return Err((StatusCode::FORBIDDEN, "Uploads ignored"));
    }
    if device_model.dongle_id != dongle_id {
        return Err((StatusCode::BAD_REQUEST, "dongle_id does not match identity"));
    }

    let upload_url = format!("{}/connectincoming/{}/{}",
        env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set"),
        device_model.dongle_id,
        transform_route_string(&params.path));
    
    tracing::info!("Device will upload to {upload_url}");
    // curl http://host/v1.4/ccfab3437bea5257/upload_url/?path=2019-06-06--11-30-31--9/fcamera.hevc&expiry_days=1
    // Assuming default expiry is 1 day if not specified
    params.validate_expiry();

    let jwt_secret = ctx.config
        .get_jwt_config()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get secrete"))?;
    let token = jwt::JWT::new(&jwt_secret.secret).generate_token(
        &(3600 * 24 as u64), 
        auth.claims.identity.to_string())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to generate token" ))?;

    Ok(Json(json!({
        "url": upload_url,
        "headers": {"Content-Type": "application/octet-stream",
                    "Authorization": format!("JWT {}", token)},
    })))
}

async fn upload_urls_handler(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(mut data): Json<UploadUrlsQuery>,
) -> impl IntoResponse {
    let device_model = if let Some(device_model) = auth.device_model{
        device_model
    } else if let Some(user_model) = auth.user_model {
        if user_model.superuser {
            DM::find_device(&ctx.db, &dongle_id)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "no device model"))?
        } else {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "no device model"))?
        }
    } else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "no device or user model"));
    };
    
    if !device_model.uploads_allowed {
        return Err((StatusCode::FORBIDDEN, "Uploads ignored"));
    }
    if device_model.dongle_id != dongle_id {
        return Err((StatusCode::BAD_REQUEST, "dongle_id does not match identity"));
    }
    
    let jwt_secret = ctx.config
        .get_jwt_config()
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get secrete"))?;
    let token = jwt::JWT::new(&jwt_secret.secret).generate_token(
        &(3600 * 24 as u64), 
        auth.claims.identity.to_string())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to generate token" ))?;

    data.validate_expiry();

    let urls: Vec<UrlResponse> = data.paths.iter().map(|path: &String| {
        UrlResponse {
            url: format!("{}/connectincoming/{dongle_id}/{}?sig={token}",
                env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set"),
                transform_route_string(path),
            ),
        }
    }).collect::<Vec<_>>();
    return Ok(format::json(urls))
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
    if let Some(user_model) = auth.user_model {
        let device_model = if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await? // Returns error if not found
        } else {
            DM::find_device(&ctx.db, &dongle_id).await?
        };
        let mut active_device_model = device_model.into_active_model();
        active_device_model.owner_id = ActiveValue::Set(None);
        active_device_model.update(&ctx.db).await?;
        format::json(UnPairResponse {success: true})
    } else {
        format::json(UnPairResponse {success: false})
    }
}

async fn device_info(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    let device = if let Some(user_model) = auth.user_model {
        if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await? // Returns error if not found
        } else {
            DM::find_device(&ctx.db, &dongle_id).await?
        }
    } else {
        auth.device_model.unwrap()
    };

    format::json(
        DeviceInfoResponse {
            dongle_id: device.dongle_id,
            alias: device.alias,
            serial: device.serial,
            last_athena_ping: device.last_athena_ping,
            ignore_uploads: !device.uploads_allowed,
            is_paired: device.owner_id.is_some(),
            public_key: device.public_key,
            prime: device.prime,
            prime_type: device.prime_type,
            trial_claimed: true,
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
    if let Some(user_model) = auth.user_model {
        if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?; // Returns error if not found
        }
        // get most recent route with gps
        let (lat, lng, time) = RM::find_latest_pos(&ctx.db, &dongle_id).await?;
        let response = DeviceLocationResponse {
            dongle_id,
            lat,
            lng,
            time,
            ..Default::default()
        };
    
        format::json(response)
    } else {
        return loco_rs::controller::bad_request("Devices can't do this");
    }

    
}

async fn device_stats(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
) -> Result<Response> {
    if let Some(user_model) = auth.user_model {
        if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?; // Returns error if not found
        }
    } else {
        if auth.device_model.unwrap().dongle_id != dongle_id {
            return loco_rs::controller::bad_request("identity does not match dongle_id in request")
        }
    }
    // Get the current time in milliseconds since the UNIX epoch
    let utc_time_now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Calculate the start of the week (7 days ago)
    let one_week_ago_millis = utc_time_now_millis - Duration::from_secs(7 * 24 * 60 * 60).as_millis() as i64;

    // Get total stats
    let (total_length, route_count, total_millis) = RM::total_length_count_and_time_filtered(
        &ctx.db,
        &dongle_id,
        None, // No time filter for total stats
        None,
    ).await?;

    // Get stats for the past week
    let (week_length, week_count, week_millis) = RM::total_length_count_and_time_filtered(
        &ctx.db,
        &dongle_id,
        Some(one_week_ago_millis), // From one week ago
        Some(utc_time_now_millis), // To now
    ).await?;


    let ret = DeviceStatsResponse{
        all: DeviceStats {
            distance: total_length,
            routes: route_count,
            minutes: (total_millis/(1000*60)) as i32
        },
        week: DeviceStats {
            distance: week_length,
            routes: week_count,
            minutes: (week_millis/(1000*60)) as i32
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
        if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?; // just error if not found
        }
    } else {
        return loco_rs::controller::bad_request("devices can't do this")
    }
    let mut route_models = RM::find_time_filtered_device_routes(&ctx.db, &dongle_id, params.start, params.end, params.limit).await?;
    route_models.retain(|route| route.maxqlog != -1); // exclude ones wher the qlog is missing
    let exp = (3600 * 24 as u64);
    let jwt_secret = ctx.config.get_jwt_config()?;
    let token = jwt::JWT::new(&jwt_secret.secret)
        .generate_token(
        &exp,
        auth.claims.identity.to_string()).unwrap_or_default();
        
    for route in route_models.iter_mut() {
        route.share_sig = token.clone();
        route.share_exp = exp.to_string();
    }


    format::json(route_models)
}

async fn route_info(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(fullname): Path<String>,
) -> Result<Response> {
    let route_model = RM::find_route(&ctx.db, &fullname).await?;
    if let Some(user_model) = auth.user_model {
        if user_model.superuser {

        } else {
            DM::find_user_device(
                &ctx.db, 
                user_model.id,
                &route_model.device_dongle_id)
                .await?; // just error if not found
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
            DM::find_user_device(
                &ctx.db, 
                user_model.id, 
                &dongle_id)
            .await?; // just error if not found
        }
    }
    let route_models = RM::find_device_routes(&ctx.db, &dongle_id).await?;
    format::json(route_models)
}


async fn get_my_devices(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // TODO: implement authorized devices!

    let user_model = auth.user_model.ok_or_else(|| (StatusCode::BAD_REQUEST, ""))?;
    let device_models = if user_model.superuser {
        DM::find_all_devices(&ctx.db).await
    } else {
        DM::find_user_devices(&ctx.db, user_model.id).await
    };
    let mut devices = vec![];
    for device_model in device_models {
        let device = DeviceResponse {
            alias: device_model.alias,
            device_type: device_model.device_type,
            dongle_id: device_model.dongle_id,
            ignore_uploads: !device_model.uploads_allowed, // flip this
            is_owner: (device_model.owner_id == Some(user_model.id)),
            is_paired: device_model.owner_id.is_some(),
            last_athena_ping: device_model.last_athena_ping,
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

    Ok(format::json(devices))
}


async fn get_me(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_model = UM::find_by_identity(&ctx.db, &auth.claims.identity).await?;
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
struct AliasJson {
    alias: String,
}

async fn update_device_alias(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Json(alias): Json<AliasJson>,
) -> impl IntoResponse {
    if auth.user_model.is_none() {
        return Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response());
    }

    let user_model = auth.user_model.unwrap();
    let device_model = if user_model.superuser {
        DM::find_device(&ctx.db, &dongle_id).await?
    } else {
        DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?
    };

    let mut active_device_model = device_model.into_active_model();
    active_device_model.alias = ActiveValue::Set(alias.alias);
    active_device_model.update(&ctx.db).await?;
    let device_model = DM::find_device(&ctx.db, &dongle_id).await?;
    format::json(
        DeviceInfoResponse {
            dongle_id: device_model.dongle_id,
            alias: device_model.alias,
            serial: device_model.serial,
            last_athena_ping: device_model.last_athena_ping,
            ignore_uploads: !device_model.uploads_allowed,
            is_paired: device_model.owner_id.is_some(),
            public_key: device_model.public_key,
            prime: device_model.prime,
            prime_type: device_model.prime_type,
            sim_id: device_model.sim_id,
            ..Default::default()
        }
    )
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
        let device_model = DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?;
        is_online = device_model.online;
        active_device = device_model.into_active_model();
    } else {
        return Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response());
    }

    let msg = JsonRpcRequest{
        method: "setNavDestination".to_string(),
        params: Some(serde_json::to_value(destination.clone())?),
        ..Default::default()
    };
    DMQM::insert_msg(&ctx.db, &dongle_id, msg).await?;

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
    //Path(dongle_id): Path<String>,
) -> impl IntoResponse {
    if auth.device_model.is_none() {
        return (StatusCode::BAD_REQUEST, format::json("Only devices can use this endpoint.")).into_response();
    }

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
        let device_model = DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?;
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
) -> Result<Response> {
    if let Some(device_model) = auth.device_model {
        let locations = device_model.locations.unwrap_or_default();
        return Ok((StatusCode::OK, Json(locations)).into_response());
    } else if let Some(user_model) = auth.user_model {
        let device_model = if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?
        } else {
            DM::find_device(&ctx.db,  &dongle_id).await?
        };
       
        let locations: Value = device_model.locations.unwrap_or_default();
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
        let device_model = if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &dongle_id).await?
        } else {
            DM::find_device(&ctx.db,  &dongle_id).await?
        };
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
    let original_len: usize = locations.len();
    locations.retain(|loc: &SavedLocation| loc.id.to_string() != payload.id);

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
        .add("/devices/:dongle_id", patch(update_device_alias))
        .add(".1/devices/:dongle_id", get(device_info))
        .add("/navigation/:dongle_id/set_destination", post(set_destination))
        .add("/navigation/:dongle_id/locations", get(get_locations).put(put_locations).delete(delete_location))
        .add("/navigation/:dongle_id/next", get(get_next_destination))
    }
