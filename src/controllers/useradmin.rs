#![allow(clippy::unused_async)]
use async_compression::tokio::bufread;
use loco_rs::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use axum::{
    extract::{Query, State}, Extension,
};
use tokio_util::io::StreamReader;
extern crate url;
use std::{collections::HashMap, env, io::Cursor};
use axum::response::{Redirect, IntoResponse};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};

use crate::{
    cereal::log_capnp::event as LogEvent, 
    common::{mkv_helpers, re::*}, 
    models::_entities, 
    views,
};

#[derive(Deserialize)]
pub struct OneBox {
    onebox: Option<String>
}

#[derive(Serialize)]
pub struct UsersTemplate {
    pub defined: bool,
    pub users: Vec<_entities::users::Model>
}
#[derive(Serialize)]
pub struct RoutesTemplate {
    pub defined: bool,
    pub routes: Vec<_entities::routes::Model>
}
#[derive(Serialize)]
pub struct DevicesTemplate {
    pub defined: bool,
    pub devices: Vec<_entities::devices::Model>
}
#[derive(Serialize)]
pub struct BootlogsTemplate {
    pub defined: bool,
    pub bootlogs: Vec<_entities::bootlogs::Model>
}

#[derive(Serialize)]
pub struct CloudlogsTemplate {
    pub defined: bool,
    // Nested summary: branch -> (map: module -> count of logs)
    pub cloudlogs: std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
}

#[derive(Serialize)]
pub struct SegmentsTemplate {
    pub defined: bool,
    pub segments: Vec<_entities::segments::Model>,
}

#[derive(Serialize, Default)]
pub struct MasterTemplate {
    pub api_host: String,
    pub ws_host: String,
    pub onebox: String,
    pub dongle_id: String,
    pub users: Option<UsersTemplate>,
    pub segments: Option<SegmentsTemplate>,
    pub devices: Option<DevicesTemplate>,
    pub routes: Option<RoutesTemplate>,
    pub bootlogs: Option<BootlogsTemplate>,
    pub cloudlogs: Option<CloudlogsTemplate>,
}

pub async fn onebox_handler(
    auth: crate::middleware::auth::MyJWT,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    Query(params): Query<OneBox>,
    Extension(manager): Extension<std::sync::Arc<super::ws::ConnectionManager>>,
) -> Result<impl IntoResponse> {
    
    let user_model = match auth.user_model {
        Some(user_model) => user_model,
        None => return unauthorized("Couldn't find user"), // Error handling for when auth.user_model is None. Should never get here
    };
    let onebox = match params.onebox {
        Some(onebox) => onebox,
        None => user_model.name.clone(),
    };
    use crate::common::re::*;
    let route_match_string = format!(
        r"^({DONGLE_ID})([_|/|]?({ROUTE_NAME}))?"
    );
    // Regex to match a complete canonical route name
    let re = regex::Regex::new(&route_match_string).unwrap();

    let mut canonical_route_name: Option<String> = None;
    let mut dongle_id: Option<String> = None;
    let mut timestamp: Option<String> = None;


    // Check for route or dongle ID
    if let Some(caps) = re.captures(&onebox) {
        dongle_id = Some(caps[1].to_string());
        if let Some(ts) = caps.get(3) {
            timestamp = Some(ts.as_str().to_string());
            canonical_route_name = Some(format!("{}|{}", dongle_id.as_ref().unwrap(), timestamp.as_ref().unwrap()));
        }
    }
    let api_endpoint: String = env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set");
    let ws_endpoint: String = env::var("WS_ENDPOINT").expect("WS_ENDPOINT env variable not set");

    let mut master_template = MasterTemplate {
        dongle_id: dongle_id.clone().unwrap_or_default(),
        onebox: onebox,
        api_host: api_endpoint,
        ws_host: ws_endpoint,
        ..Default::default()
    };
    if user_model.superuser {
        master_template.users = Some(UsersTemplate {
            defined: true,
            users: _entities::users::Model::find_all_users(&ctx.db).await
        });
    } else {
        master_template.users = Some(UsersTemplate {
            defined: true,
            users: vec![user_model.clone()],
        });
    }

    // Parse cloudlogs from the connection manager's cache and add them to the template if available.
    if let Some(ref d_id) = dongle_id {
        // Retrieve the nested logs from the cloudlog cache.
        // Here we assume the cache is stored as:
        // HashMap<String, HashMap<String, HashMap<String, Vec<Value>>>>
        // where the outer key is the device id, then branch, then module.
        let nested_logs: Option<HashMap<String, HashMap<String, Vec<Value>>>> = {
            let cloudlog_cache = manager.cloudlog_cache.read().await;
            cloudlog_cache.get(d_id).cloned()
        };
    
        // Build a summary: for each branch and module, count the number of logs.
        let summary: HashMap<String, HashMap<String, usize>> =
            if let Some(nested) = nested_logs {
                let mut map = HashMap::new();
                for (branch, modules) in nested {
                    let mut module_map = HashMap::new();
                    for (module, logs_array) in modules {
                        module_map.insert(module, logs_array.len());
                    }
                    map.insert(branch, module_map);
                }
                map
            } else {
                HashMap::new()
            };
    
        master_template.cloudlogs = Some(CloudlogsTemplate {
            defined: !summary.is_empty(),
            cloudlogs: summary,
        });
    }

    if let Some(canonical_route) = canonical_route_name {
        let mut segment_models = Some(_entities::segments::Model::find_segments_by_route(&ctx.db, &canonical_route).await?);
        if let Some(segment_models) = segment_models.as_mut() {
            segment_models.sort_by(|a, b| a.number.cmp(&b.number));
        }

        master_template.segments = segment_models.map(|segments| SegmentsTemplate { 
            defined: true, 
            segments 
        });
    
        views::route::admin_route(v, master_template)
    } else if let Some(d_id) = dongle_id {
        master_template.routes = Some(RoutesTemplate { 
            defined: true, 
            routes: _entities::routes::Model::find_device_routes(&ctx.db, &d_id).await?, 
        });
        master_template.devices = Some(DevicesTemplate {
            defined: true,
            devices: _entities::devices::Model::find_user_devices(&ctx.db, user_model.id).await,
        });
        master_template.bootlogs = Some(BootlogsTemplate {
            defined: true,
            bootlogs: _entities::bootlogs::Model::find_device_bootlogs(&ctx.db, &d_id).await?,
        });
        views::route::admin_route(v, master_template)

    } else {
        if user_model.superuser {
            master_template.devices = Some(DevicesTemplate {
                defined: true,
                devices: _entities::devices::Model::find_all_devices(&ctx.db).await
            });
        } else {
            master_template.devices = Some(DevicesTemplate {
                defined: true,
                devices: _entities::devices::Model::find_user_devices(&ctx.db, user_model.id).await
            });

        };
        // Fallback response
        views::route::admin_route(v, master_template)
    }
}


// A function that uses capnp to parse the qlog file and return the parsed data
#[derive(Deserialize)]
pub struct UlogQuery {
    pub url: String,
    pub event: Option<String>
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String,
   pub events: Vec<String>,
   pub selected_event: Option<String>,
}


pub async fn qlog_render(
    _auth: crate::middleware::auth::MyJWT, // Using underscore to indicate it's required but not used
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
    Extension(client): Extension<Client>,
    Query(params): Query<UlogQuery>
) -> Result<impl IntoResponse> {
    // Validate the URL
    let segment_file_regex_string = format!(
        r"(({DONGLE_ID})_({ROUTE_NAME})--({NUMBER})--({ALLOWED_FILENAME}$))"
    );
    let segment_file_regex = regex::Regex::new(&segment_file_regex_string).unwrap();
    let response = if let Some(captures) = segment_file_regex.captures(&params.url) {
        // Always use mkv_helpers::get_mkv_file_url with the second part (lookup key)
        let internal_file_url = mkv_helpers::get_mkv_file_url(&captures[0]);
        
        // Proceed with the request using the `internal_file_url`
        let request = client.get(&internal_file_url);
    
        // Get the data and save it as a string to pass to admin_segment_ulog
        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    response
                } else {
                    return Err(Error::Message(format!("Failed to get file: {}", response.status())));
                }
            }
            Err(e) => {
                return Err(Error::Message(format!("Failed to get file: {}", e)));
            }
        }
    } else {
        return Err(Error::Message("Invalid file name".to_string()));
    };
                
    // Decompress the file
    use futures_util::TryStreamExt;
    let bytes_stream = response
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    
    let stream_reader = StreamReader::new(bytes_stream);
    
    let file_name = params.url.clone();
    let mut decoder: std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>> = if file_name.ends_with(".bz2") {
        Box::pin(bufread::BzDecoder::new(stream_reader))
    } else if file_name.ends_with(".zst") {
        Box::pin(bufread::ZstdDecoder::new(stream_reader))
    } else {
        return Err(Error::Message("Invalid file type. Must end with .bz2 or .zst".to_string()));
    };
    
    let mut decompressed_data = Vec::new();
    match tokio::io::AsyncReadExt::read_to_end(&mut decoder, &mut decompressed_data).await {
        Ok(_) => (),
        Err(e) => return Err(Error::Message(e.to_string()))
    };

    let mut cursor = Cursor::new(decompressed_data);
    let mut unlog_data = Vec::new();
    let reader_options = capnp::message::ReaderOptions::default();
    
    // Create a set to store event names
    let mut event_types = std::collections::HashSet::new();

    use std::io::Write;
    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, reader_options) {
        let event = match message_reader.get_root::<LogEvent::Reader>() {
            Ok(event) => event,
            Err(e) => {
                tracing::warn!("Failed to get root: {:?}", e); 
                continue;
            }, 
        };
        
        match event.which() {
            Err(_) => {
                continue;
            }
            Ok(event_type) => {
                // Get the string representation of the event type
                let type_name = crate::common::types::get_event_name(&event_type);
                event_types.insert(type_name.clone());
                // If an event is requested, only output that event's data
                if let Some(ref requested_event) = params.event {
                    if type_name == *requested_event {
                        writeln!(&mut unlog_data, "{:#?}", event).unwrap_or(());
                    }
                }
            }
        }
    }
    
    // Always return the list of events
    let mut event_list: Vec<String> = event_types.into_iter().collect();
    event_list.sort();

    let data = if let Some(_) = params.event {
        String::from_utf8(unlog_data).unwrap_or_else(|_| "Failed to convert log data to string".to_string())
    } else if !event_list.is_empty() {
        format!("Available event types:\n{}", event_list.join("\n"))
    } else {
        "No events found in log".to_string()
    };

    Ok(views::route::admin_segment_ulog(v, UlogText {
        text: data,
        events: event_list,
        selected_event: params.event.clone(),
    }))
}

pub async fn cloudlogs_view(
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
) -> Result<impl IntoResponse> {
    views::route::admin_cloudlogs(v, CloudlogsTemplate {
        defined: true,
        cloudlogs: HashMap::new(),
    })
}

pub async fn login(
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
) -> Result<impl IntoResponse> {
    views::auth::login(
        v, 
        crate::views::auth::LoginTemplate { 
            api_host: env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set")
        }
    )
}

pub async fn logout() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    // Expire the jwt cookie
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_static("jwt=; Path=/; HttpOnly; Secure; Max-Age=0; SameSite=Lax;")
    );
    // Redirect to login page
    (StatusCode::FOUND, headers, Redirect::to("/login"))
}

pub async fn param_stats(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse> {
    // Load the param stats file (e.g., Model.json)
    let bytes = ctx.storage.download::<Vec<u8>>(std::path::Path::new("params/Model.json")).await?;
    let value_counts: HashMap<String, u64> = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| HashMap::new());

    #[derive(Serialize)]
    struct ParamStatsTemplate {
        param_name: String,
        values: Vec<(String, u64)>,
    }

    let mut values: Vec<_> = value_counts.into_iter().collect();
    values.sort_by(|a, b| b.1.cmp(&a.1)); // Sort descending by count

    let template = ParamStatsTemplate {
        param_name: "Model".to_string(),
        values,
    };

    Ok(v.render("stats/param_stats.html", &template))
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/", get(onebox_handler))
        .add("/login", get(login))
        .add("/cloudlogs", get(cloudlogs_view))
        .add("/qlog", get(qlog_render))
        .add("/auth/logout", get(logout))
        .add("/params_stats", get(param_stats))
}
