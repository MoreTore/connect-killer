#![allow(clippy::unused_async)]
use std::{collections::HashMap, sync::Arc};

use loco_rs::prelude::*;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get, Extension,

  };
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::Value as JsonValue;
use crate::{
    common::{
        re::*,
        mkv_helpers
    },
    enforce_ownership_rule,
    models::{
        devices::DM,
        routes::RM,
    }
};

use super::ws::ConnectionManager;
use chrono::{Timelike, Datelike, Utc, TimeZone};

// Helper function to sanitize control characters within JSON string literals
fn sanitize_json_string_content(data: &str) -> String {
    let mut sanitized = String::with_capacity(data.len());
    let mut in_string = false;
    let mut last_char_was_escape = false;

    for c in data.chars() {
        if last_char_was_escape {
            sanitized.push(c);
            last_char_was_escape = false;
            continue;
        }

        if c == '\\' { // Backslash character
            sanitized.push(c);
            last_char_was_escape = true;
            continue;
        }

        if c == '\"' { // Double quote character
            in_string = !in_string;
            sanitized.push(c);
            continue;
        }

        if in_string {
            if c.is_control() {
                // Standard JSON escapes for common control characters
                match c {
                    '\u{0008}' => sanitized.push_str("\\\\b"), // Backspace
                    '\u{000C}' => sanitized.push_str("\\\\f"), // Form feed
                    '\u{000A}' => sanitized.push_str("\\\\n"), // Line feed
                    '\u{000D}' => sanitized.push_str("\\\\r"), // Carriage return
                    '\u{0009}' => sanitized.push_str("\\\\t"), // Tab
                    _ => {
                        // For other control characters, use \\uXXXX unicode escape
                        sanitized.push_str(&format!("\\\\u{:04x}", c as u32));
                    }
                }
            } else {
                sanitized.push(c);
            }
        } else {
            // Outside a string, push character as is.
            sanitized.push(c);
        }
    }
    sanitized
}


#[derive(Deserialize)]
pub struct UlogQuery {
    pub url: String
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String
}

#[derive(Serialize)]
pub struct UlogTopics {
    pub text: String,
    pub topics: Vec<String>,
    pub url: String,
}

#[derive(Deserialize)]
pub struct LogListQuery {
    pub event: String,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Serialize)]
pub struct LogListResponse {
    pub keys: Vec<String>,
    pub has_more: bool,
}

#[derive(Deserialize)]
pub struct LogEntryQuery {
    pub event: String,
    pub key: String,
}

#[derive(Serialize)]
pub struct LogEntryResponse {
    pub key: String,
    pub value: JsonValue,
}

#[derive(Deserialize)]
pub struct SegmentLogQuery {
    pub log_key: String, // e.g. "abc123_00000000--c0ffee--0--events.json" or similar
    pub tag: String,     // e.g. "accelerometer"
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Serialize)]
pub struct SegmentLogResponse {
    pub entries: Vec<serde_json::Value>,
    pub has_more: bool,
}

pub async fn asset_download(
    lookup_key: String,
    client: &reqwest::Client,
    headers: HeaderMap,
) -> Result<Response<reqwest::Body>, (StatusCode, &'static str)> {
    // Validate the lookup_key to allow only alphanumeric, '_', '-' and '.' characters
    let valid_key_pattern = regex::Regex::new(r"^[a-zA-Z0-9_.-]+$").unwrap();
    if !valid_key_pattern.is_match(&lookup_key) {
        return Err((StatusCode::BAD_REQUEST, "Invalid lookup key"));
    }
    let internal_file_url = mkv_helpers::get_mkv_file_url(&lookup_key);

    // Prepare a request to fetch the file from storage
    let mut request_builder = client.get(&internal_file_url);

    // Check for range header and forward it if present
    if let Some(range) = headers.get(hyper::header::RANGE) { 
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
    };

    let res = request_builder.send().await;

    match res {
        Ok(response) => {
            let status = response.status();
            // Build the response, copying all headers from the backend
            let mut response_builder = Response::builder().status(status);
            for (key, value) in response.headers().iter() {
                // Skip hop-by-hop headers (e.g., transfer-encoding)
                if key == hyper::header::TRANSFER_ENCODING {
                    continue;
                }
                response_builder = response_builder.header(key, value);
            }
            // Optionally add/override cache control and content disposition
            response_builder = response_builder
                .header(
                    hyper::header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{lookup_key}\"")
                )
                .header(hyper::header::CACHE_CONTROL, "public, max-age=31536000");
            let body = reqwest::Body::wrap_stream(response.bytes_stream());
            let proxy_response = response_builder.body(body).unwrap();
            Ok(proxy_response)
        }
        Err(_) => Err((StatusCode::BAD_GATEWAY, "Internal server error")),
    }
}

pub async fn ensure_user_is_owner(
    auth: &crate::middleware::auth::MyJWT,
    db: &DatabaseConnection,
    dongle_id: &str
) -> Result<(),(StatusCode,&'static str)> {
    // If the request comes from a user, check their permissions
    if let Some(user_model) = &auth.user_model {
        if !user_model.superuser {
            DM::ensure_user_device(
                &db,
                user_model.id,
                &dongle_id)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "You are not the owner"))?;
        }
    } else {
        return Err((StatusCode::FORBIDDEN, "Devices can't do this"));
    }
    Ok(())
}

pub async fn is_owner(
    auth: &crate::middleware::auth::MyJWT,
    db: &DatabaseConnection,
    dongle_id: &str
) -> bool {
    if let Some(user_model) = &auth.user_model {
        if !user_model.superuser {
            return DM::find_user_device(&db, user_model.id, &dongle_id)
                .await
                .map(|_| true)
                .unwrap_or(false);
        }
    }
    false
}

pub async fn events_download(
    Path((_dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--events.json");  // canonical_route_name include dongleid already
    return asset_download(lookup_key, &client, headers).await;
}

pub async fn coords_download(
    Path((_dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--coords.json");  // canonical_route_name include dongleid already
    return asset_download(lookup_key, &client, headers).await;
}

pub async fn sprite_download(
    Path((_dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--sprite.jpg");  // canonical_route_name include dongleid already
    return asset_download(lookup_key, &client, headers).await;
}

pub async fn auth_file_download(
    auth: crate::middleware::auth::MyJWT,
    Path((dongle_id, route_name, segment, file)): Path<(String, String, String, String)>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let owner = is_owner(&auth, &ctx.db, &dongle_id).await;
    let superuser = auth.user_model.map(|u| u.superuser).unwrap_or(false);
    let fullname = format!("{dongle_id}|{route_name}", dongle_id=dongle_id, route_name=route_name);
    let public = RM::is_public(&ctx.db, &fullname).await.unwrap_or(false);
    if !public && !owner && !superuser {
        return Err((StatusCode::UNAUTHORIZED, "You are not the owner and the route is not public"));
    }

    let lookup_key = format!("{dongle_id}_{route_name}--{segment}--{file}");
    return asset_download(lookup_key, &client, headers).await;
}

pub async fn bootlog_file_download(
    auth: crate::middleware::auth::MyJWT,
    Path(bootlog_file): Path<String>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let bootlog_re_string = format!(r"({DONGLE_ID})_boot_{ROUTE_NAME}");
    let re = regex::Regex::new(&bootlog_re_string).unwrap();

    // Check if the bootlog_file contains the pattern using regex
    if let Some(captures) = re.captures(&bootlog_file) {
        // Extract the dongle_id from the captures
        if let Some(dongle_id) = captures.get(1) {
            let dongle_id = dongle_id.as_str();

            // Pass the captured dongle_id to ensure_user_is_owner
            ensure_user_is_owner(&auth, &ctx.db, dongle_id).await?;
            return asset_download(bootlog_file, &client, headers).await;
        }
    }

    // If the regex does not match or dongle_id is not captured, return an error
    Err((StatusCode::BAD_REQUEST, "Invalid bootlog format"))
}

// TODO Migrate DB to remove the redundant file_type path
pub async fn depreciated_auth_file_download(
    auth: crate::middleware::auth::MyJWT,
    Path((_file, dongle_id, route_name, segment, file)): Path<(String, String, String, String, String)>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    ensure_user_is_owner(&auth, &ctx.db, &dongle_id).await?;
    let lookup_key = format!("{dongle_id}_{route_name}--{segment}--{file}");  // Note that route_name does not include dongle_id
    return asset_download(lookup_key, &client, headers).await;
}


async fn delete_route(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path((dongle_id, timestamp)): Path<(String, String)>,
    Extension(client): Extension<reqwest::Client>,
) -> impl IntoResponse {
    let dongle_id = if let Some(device_model) = auth.device_model {
        device_model.dongle_id
    } else {
        dongle_id
    };

    if let Some(user_model) = auth.user_model {
        let device_model = DM::find_device(&ctx.db, &dongle_id).await?;
        if !user_model.superuser {
            enforce_ownership_rule!(
                user_model.id,
                device_model.owner_id,
                "Can only delete your own devices data!"
            );
        }
    }
    let canonical_route_name = format!("{}|{}", &dongle_id, &timestamp);
    RM::delete_route(&ctx.db, &canonical_route_name).await?; // should cascade to segments

    let query = mkv_helpers::list_keys_starting_with(&canonical_route_name.replace("|", "_"));
    let response = client.get(&query).send().await.unwrap();
    if !response.status().is_success() {
        tracing::info!("Failed to get keys");
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get keys").into_response());
    }
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)?; // Convert response text into JSON

    let keys = json["keys"].as_array().unwrap();
    tracing::info!("Deleting {} files from kv store", keys.len());

    for key_value in keys {
        let file_name = key_value.as_str().unwrap().trim_start_matches('/').to_string(); // Convert to string for independent ownership
        let file_url = mkv_helpers::get_mkv_file_url(&file_name);

        let _ = client
            .delete(&file_url)
            .send()
            .await
            .map_err(|e|
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            );
    }

    return Ok((StatusCode::OK, format!("Deleted {} files", keys.len())).into_response());
}

async fn delete_data(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Extension(client): Extension<reqwest::Client>,
) -> impl IntoResponse {

    let dongle_id = if let Some(device_model) = auth.device_model {
        device_model.dongle_id
    } else {
        dongle_id
    };

    if let Some(user_model) = auth.user_model {
        let device_model = DM::find_device(&ctx.db, &dongle_id).await?;
        if !user_model.superuser {
            enforce_ownership_rule!(
                user_model.id,
                device_model.owner_id,
                "Can only delete your own devices data!"
            );
        }
        let mut active_device_model = device_model.into_active_model();
        active_device_model.server_storage = ActiveValue::Set(0);
        active_device_model.locations = ActiveValue::Set(None);
        active_device_model.alias = ActiveValue::Set("".to_string());
        active_device_model.owner_id = ActiveValue::Set(None);
        active_device_model.update(&ctx.db).await?;
    }

    let query = mkv_helpers::list_keys_starting_with(&dongle_id);
    let response = client.get(&query).send().await.unwrap();
    if !response.status().is_success() {
        tracing::info!("Failed to get keys");
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get keys").into_response());
    }
    let body = response.text().await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&body)?; // Convert response text into JSON

    let keys = json["keys"].as_array().unwrap();
    tracing::info!("Deleting {} files from kv store", keys.len());

    for key_value in keys {
        let file_name = key_value.as_str().unwrap().trim_start_matches('/').to_string(); // Convert to string for independent ownership
        let file_url = mkv_helpers::get_mkv_file_url(&file_name);

        let _ = client
            .delete(&file_url)
            .send()
            .await
            .map_err(|e|
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            );
    }
    // We cant delete the device model but we still want to delete all the routes and segments. We want to keep the device in the db so it can
    // be used for the new customer that pairs the device.
    RM::delete_device_routes(&ctx.db, &dongle_id).await?;
    return Ok((StatusCode::OK, format!("Deleted {} files", keys.len())).into_response());
}


#[derive(Deserialize)]
struct CloudlogQuery {
    branch: Option<String>,
    module: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn get_cloudlog_cache(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Path(dongle_id): Path<String>,
    Query(query): Query<CloudlogQuery>,
    Extension(manager): Extension<Arc<ConnectionManager>>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Authorization check
    if let Some(user_model) = auth.user_model {
        let device_model = DM::find_device(&ctx.db, &dongle_id)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Device not found"))?;
        if !user_model.superuser {
            if let Some(owner_id) = device_model.owner_id {
                if user_model.id != owner_id {
                    return Err((StatusCode::UNAUTHORIZED, "You are not the owner"));
                }
            } else {
                return Err((StatusCode::UNAUTHORIZED, "Only registered devices can upload"));
            }
        }
    } else {
        return Err((StatusCode::FORBIDDEN, "Devices can't do this"));
    }

    let cloudlog_cache = manager.cloudlog_cache.read().await;
    if let Some(device_logs) = cloudlog_cache.get(&dongle_id) {
        // device_logs: HashMap<String, HashMap<String, Vec<Value>>>
        if let (Some(branch), Some(module)) = (query.branch, query.module) {
            if let Some(module_logs) = device_logs.get(&branch).and_then(|m| m.get(&module)) {
                let offset = query.offset.unwrap_or(0);
                let limit = query.limit.unwrap_or(50);
                let sliced: Vec<Value> = module_logs
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .cloned()
                    .collect();
                let json_bytes = serde_json::to_vec(&sliced).map_err(|_| {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize cloudlogs")
                })?;
                return Ok((
                    StatusCode::OK,
                    [("Content-Type", "application/json")],
                    json_bytes,
                ));
            } else {
                return Err((
                    StatusCode::NOT_FOUND,
                    "No logs found for the specified branch/module",
                ));
            }
        } else {
            // If branch or module is not provided, return a summary structure.
            // For example, a mapping from branch -> list of module names.
            let summary: HashMap<&String, Vec<&String>> = device_logs
                .iter()
                .map(|(branch, modules)| (branch, modules.keys().collect()))
                .collect();
            let json_bytes = serde_json::to_vec(&summary).map_err(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize summary")
            })?;
            return Ok((
                StatusCode::OK,
                [("Content-Type", "application/json")],
                json_bytes,
            ));
        }
    } else {
        Err((StatusCode::NOT_FOUND, "No cloudlog data found for this dongle"))
    }
}

#[derive(Deserialize)]
pub struct CloudlogAllQuery {
    pub dongle_id: Option<String>,
    pub branch: Option<String>,
    pub module: Option<String>,
    pub level: Option<String>,
    pub levelnum: Option<u64>,
    pub func_name: Option<String>,
    pub date_from: Option<f64>, // unix timestamp (seconds)
    pub date_to: Option<f64>,   // unix timestamp (seconds)
    pub minute_from: Option<u32>,
    pub minute_to: Option<u32>,
    pub second_from: Option<u32>,
    pub second_to: Option<u32>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
    pub hour: Option<u32>,
    pub minute: Option<u32>,
    pub second: Option<u32>,
    // ctx fields
    pub ctx_branch: Option<String>,
    pub ctx_commit: Option<String>,
    pub ctx_device: Option<String>,
    pub ctx_dirty: Option<bool>,
    pub ctx_dongle_id: Option<String>,
    pub ctx_origin: Option<String>,
    pub ctx_version: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

pub async fn get_all_cloudlogs(
    auth: crate::middleware::auth::MyJWT,
    State(_ctx): State<AppContext>,
    Query(mut query): Query<CloudlogAllQuery>,
    Extension(manager): Extension<Arc<ConnectionManager>>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Only allow superusers for all-devices query
    // Allow specific users to access all logs for FrogPilot-Testing branch or any branch that starts with "FrogPilot"
    let frogai_id = Uuid::parse_str("f0b1a2c3-d4e5-6789-abcd-ef0123456789").unwrap();
    let is_frogai = auth.user_model.as_ref().map_or(false, |u| u.identity == frogai_id);
    if let Some(user_model) = &auth.user_model {
        if is_frogai {
            if let Some(ref filter_branch) = query.branch {
                if !filter_branch.starts_with("FrogPilot") {
                    return Err((StatusCode::UNAUTHORIZED, "You are not authorized to access all cloudlogs"));
                }
            } else {
                // force the branch to be "FrogPilot-Testing" for FrogAI
                query.branch = Some("FrogPilot-Testing".to_string());
            }
        } else if !user_model.superuser {
            return Err((StatusCode::UNAUTHORIZED, "You are not authorized to access all cloudlogs"));
        }
    } else {
        return Err((StatusCode::FORBIDDEN, "Devices can't do this"));
    }

    let cloudlog_cache = manager.cloudlog_cache.read().await;
    let mut results = Vec::new();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let mut count = 0;
    let mut skipped = 0;

    for (dongle_id, device_logs) in cloudlog_cache.iter() {
        if let Some(ref filter_dongle) = query.dongle_id {
            if dongle_id != filter_dongle { continue; }
        }
        for (branch, modules) in device_logs.iter() {
            if let Some(ref filter_branch) = query.branch {
                if branch != filter_branch { continue; }
            }
            for (module, logs) in modules.iter() {
                if let Some(ref filter_module) = query.module {
                    if module != filter_module { continue; }
                }
                for log in logs.iter() {
                    // Filter by level
                    if let Some(ref filter_level) = query.level {
                        if let Some(level) = log.get("level").and_then(|v| v.as_str()) {
                            if level != filter_level { continue; }
                        } else { continue; }
                    }
                    // Filter by levelnum
                    if let Some(filter_levelnum) = query.levelnum {
                        if let Some(levelnum) = log.get("levelnum").and_then(|v| v.as_u64()) {
                            if levelnum != filter_levelnum { continue; }
                        } else { continue; }
                    }
                    // Filter by funcName (now func_name)
                    if let Some(ref filter_func) = query.func_name {
                        if let Some(func) = log.get("funcName").and_then(|v| v.as_str()) {
                            if func != filter_func { continue; }
                        } else { continue; }
                    }
                    // Filter by date
                    if let Some(created) = log.get("created").and_then(|v| v.as_f64()) {
                        if let Some(date_from) = query.date_from {
                            if created < date_from { continue; }
                        }
                        if let Some(date_to) = query.date_to {
                            if created > date_to { continue; }
                        }
                        // Updated chrono usage
                        let dt = Utc.timestamp_opt(created as i64, (created.fract() * 1e9) as u32).single().map(|dt_utc| dt_utc.naive_utc());
                        if let Some(dt) = dt {
                            if let Some(year) = query.year {
                                if dt.year() != year { continue; }
                            }
                            if let Some(month) = query.month {
                                if dt.month() != month { continue; }
                            }
                            if let Some(day) = query.day {
                                if dt.day() != day { continue; }
                            }
                            if let Some(hour) = query.hour {
                                if dt.hour() != hour { continue; }
                            }
                            if let Some(minute) = query.minute {
                                if dt.minute() != minute { continue; }
                            }
                            if let Some(second) = query.second {
                                if dt.second() != second { continue; }
                            }
                            if let Some(minute_from) = query.minute_from {
                                if dt.minute() < minute_from { continue; }
                            }
                            if let Some(minute_to) = query.minute_to {
                                if dt.minute() > minute_to { continue; }
                            }
                            if let Some(second_from) = query.second_from {
                                if dt.second() < second_from { continue; }
                            }
                            if let Some(second_to) = query.second_to {
                                if dt.second() > second_to { continue; }
                            }
                        } else { continue; } // If timestamp is invalid, skip
                    }
                    // Filter by ctx fields
                    if let Some(ctx) = log.get("ctx") {
                        if let Some(ref ctx_branch) = query.ctx_branch {
                            if ctx.get("branch").and_then(|v| v.as_str()) != Some(ctx_branch) { continue; }
                        }
                        if let Some(ref ctx_commit) = query.ctx_commit {
                            if ctx.get("commit").and_then(|v| v.as_str()) != Some(ctx_commit) { continue; }
                        }
                        if let Some(ref ctx_device) = query.ctx_device {
                            if ctx.get("device").and_then(|v| v.as_str()) != Some(ctx_device) { continue; }
                        }
                        if let Some(ctx_dirty) = query.ctx_dirty {
                            if ctx.get("dirty").and_then(|v| v.as_bool()) != Some(ctx_dirty) { continue; }
                        }
                        if let Some(ref ctx_dongle_id) = query.ctx_dongle_id {
                            if ctx.get("dongle_id").and_then(|v| v.as_str()) != Some(ctx_dongle_id) { continue; }
                        }
                        if let Some(ref ctx_origin) = query.ctx_origin {
                            if ctx.get("origin").and_then(|v| v.as_str()) != Some(ctx_origin) { continue; }
                        }
                        if let Some(ref ctx_version) = query.ctx_version {
                            if ctx.get("version").and_then(|v| v.as_str()) != Some(ctx_version) { continue; }
                        }
                    } else {
                        // If ctx filter is set but log has no ctx, skip
                        if query.ctx_branch.is_some() || query.ctx_commit.is_some() || query.ctx_device.is_some() || query.ctx_dirty.is_some() || query.ctx_dongle_id.is_some() || query.ctx_origin.is_some() || query.ctx_version.is_some() {
                            continue;
                        }
                    }
                    // Pagination
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    if count >= limit {
                        break;
                    }
                    // Attach context for device, branch, module
                    let mut log_with_ctx = log.clone();
                    log_with_ctx["_dongle_id"] = serde_json::Value::String(dongle_id.clone());
                    log_with_ctx["_branch"] = serde_json::Value::String(branch.clone());
                    log_with_ctx["_module"] = serde_json::Value::String(module.clone());
                    results.push(log_with_ctx);
                    count += 1;
                }
                if count >= limit { break; }
            }
            if count >= limit { break; }
        }
        if count >= limit { break; }
    }
    let json_bytes = serde_json::to_vec(&results).map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize cloudlogs")
    })?;
    Ok((StatusCode::OK, [("Content-Type", "application/json")], json_bytes))
}

// List top-level keys (sorted, paginated) with auth and pattern check
pub async fn api_useradmin_log_keys(
    auth: crate::middleware::auth::MyJWT,
    Query(query): Query<LogListQuery>,
    Extension(client): Extension<reqwest::Client>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Only allow superusers
    if !auth.user_model.as_ref().map(|u| u.superuser).unwrap_or(false) {
        return Err((StatusCode::UNAUTHORIZED, "Not authorized"));
    }
    // Validate event name
    let valid_key_pattern = regex::Regex::new(r"^[a-zA-Z0-9_.-]+$").unwrap();
    if !valid_key_pattern.is_match(&query.event) {
        return Err((StatusCode::BAD_REQUEST, "Invalid event name"));
    }
    let page = query.page.unwrap_or(0);
    let page_size = query.page_size.unwrap_or(100);
    let log_key = format!("{}.log", query.event);
    let url = mkv_helpers::get_mkv_file_url(&log_key);
    let resp = client.get(&url).send().await.map_err(|_| (StatusCode::BAD_GATEWAY, "Failed to fetch log file"))?;
    if !resp.status().is_success() {
        return Err((StatusCode::NOT_FOUND, "Log file not found"));
    }
    let data = resp.text().await.map_err(|_| (StatusCode::BAD_GATEWAY, "Failed to read log file"))?;
    let json: JsonValue = serde_json::from_str(&data).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid JSON"))?;
    let obj = json.as_object().ok_or((StatusCode::BAD_REQUEST, "Expected JSON object at top level"))?;
    let mut keys: Vec<String> = obj.keys().cloned().collect();
    keys.sort();
    let start = page * page_size;
    let end = (start + page_size).min(keys.len());
    let has_more = end < keys.len();
    let page_keys = keys[start..end].to_vec();
    Ok(axum::Json(LogListResponse { keys: page_keys, has_more }))
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("connectdata")
        .add("/:dongle_id/:timestamp/:segment/coords.json", get(coords_download))
        .add("/:dongle_id/:timestamp/:segment/events.json", get(events_download))
        .add("/:dongle_id/:timestamp/:segment/sprite.jpg", get(sprite_download))
        .add("/:dongle_id/:timestamp/:segment/:file", get(auth_file_download))
        .add("/:filetype/:dongle_id/:timestamp/:segment/:file", get(depreciated_auth_file_download))
        .add("/delete/:dongle_id", delete(delete_data))
        .add("/delete/:dongle_id/:timestamp", delete(delete_route))
        .add("/bootlog/:bootlog_file", get(bootlog_file_download))
        .add("/:dongle_id/cloudlogs", get(get_cloudlog_cache))
        .add("/cloudlogs/all", get(get_all_cloudlogs))
}
