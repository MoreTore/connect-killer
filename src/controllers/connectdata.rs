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
use crate::{
    common::{
        re::*,
        mkv_helpers
    },
    enforce_ownership_rule,
    models::{
        devices::DM,
        routes::RM,
        segments::SM,
        device_msg_queues::DMQM,
        bootlogs::BM,
    }
};

use super::ws::ConnectionManager;


#[derive(Deserialize)]
pub struct UlogQuery {
    pub url: String
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String
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
    let range_header_present = if let Some(range) = headers.get(hyper::header::RANGE) {
        request_builder = request_builder.header(hyper::header::RANGE, range.clone());
        true
    } else {
        false
    };

    let res = request_builder.send().await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                let content_length = response.headers().get(hyper::header::CONTENT_LENGTH)
                    .and_then(|ct_len| ct_len.to_str().ok())
                    .and_then(|ct_len| ct_len.parse::<u64>().ok());

                // Build the response with additional caching header
                let mut response_builder = Response::builder();
                response_builder = response_builder
                    .header(
                        hyper::header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{lookup_key}\"")
                    )
                    // Set a long max-age so the browser caches the sprite
                    .header(hyper::header::CACHE_CONTROL, "public, max-age=31536000");

                // Add Content-Length if available
                if let Some(length) = content_length {
                    response_builder = response_builder.header(hyper::header::CONTENT_LENGTH, length);
                }

                let body = reqwest::Body::wrap_stream(response.bytes_stream());
                if range_header_present {
                    response_builder = response_builder.status(StatusCode::PARTIAL_CONTENT);
                } else {
                    response_builder = response_builder.status(StatusCode::OK);
                }
                let proxy_response = response_builder.body(body).unwrap();

                Ok(proxy_response)
            } else {
                Err((StatusCode::from(response.status()), "Failed to fetch the file"))
            }
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
    Path((dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--events.json");  // canonical_route_name include dongleid already
    return asset_download(lookup_key, &client, headers).await;
}

pub async fn coords_download(
    Path((dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lookup_key = format!("{canonical_route_name}--{segment}--coords.json");  // canonical_route_name include dongleid already
    return asset_download(lookup_key, &client, headers).await;
}

pub async fn sprite_download(
    Path((dongle_id, canonical_route_name, segment)): Path<(String, String, String)>,
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
    let fullname = format!("{dongle_id}|{route_name}", dongle_id=dongle_id, route_name=route_name);
    let public = RM::is_public(&ctx.db, &fullname).await.unwrap_or(false);
    if !public && !owner {
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

pub async fn render_segment_ulog(
    auth: crate::middleware::auth::MyJWT,
    ViewEngine(v): ViewEngine<TeraView>, 
    State(_ctx): State<AppContext>,
    Extension(client): Extension<reqwest::Client>,
    Query(params): Query<UlogQuery>
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    // Validate the lookup_key to allow only alphanumeric, '_', '-' and '.' characters
    let valid_key_pattern = regex::Regex::new(r"^(http://localhost:3000/|https://localhost:3000/)?([a-zA-Z0-9_.-]+)$").unwrap(); //TODO purge hardcoded url from db
    if let Some(captures) = valid_key_pattern.captures(&params.url) {
        // Always use mkv_helpers::get_mkv_file_url with the second part (lookup key)
        let internal_file_url = mkv_helpers::get_mkv_file_url(&captures[2]);
    
        // Proceed with the request using the `internal_file_url`
        let request = client.get(&internal_file_url);
    
        // Get the data and save it as a string to pass to admin_segment_ulog
        let res = request.send().await;
    
        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let bytes = response.bytes().await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Error reading response"))?;
                    let bytes_vec: Vec<u8> = bytes.to_vec(); // Convert bytes to Vec<u8>
                    let data = unsafe { String::from_utf8_unchecked(bytes_vec) };
    
                    // Render the view with the fetched data
                    Ok(crate::views::route::admin_segment_ulog(v, UlogText { text: data }))
                } else {
                    Err((StatusCode::BAD_GATEWAY, "Failed to fetch the segment"))
                }
            }
            Err(_) => Err((StatusCode::BAD_GATEWAY, "Failed to fetch the segment")),
        }
    } else {
        Err((StatusCode::BAD_REQUEST, "Invalid lookup key"))
    }
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
    // Do this last in case the above fails for some reason
    let device_model = DM::find_device(&ctx.db, &dongle_id).await?;
    device_model.delete(&ctx.db).await?; // should cascade to all related data

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
        .add("/logs/", get(render_segment_ulog))
        .add("/bootlog/:bootlog_file", get(bootlog_file_download))
        .add("/:dongle_id/cloudlogs", get(get_cloudlog_cache))
}
