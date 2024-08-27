#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{ws::{Message, 
                   WebSocket, 
                   WebSocketUpgrade}, Path 
             }, http::HeaderMap, response::IntoResponse, routing::get, Extension 
};
use futures::stream::SplitSink;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{self, Duration};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    enforce_ownership_rule, 
    models::_entities::{
        devices, 
        users
    }
};

#[derive(Deserialize, Serialize)]
struct JsonRpcRequest {
    method: String,
    params: Option<serde_json::Value>,
    jsonrpc: String,
    id: u64,
}

#[derive(Deserialize, Serialize)]
struct JsonRpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    jsonrpc: String,
    id: u64,
}

pub struct ConnectionManager {
    devices: Mutex<HashMap<String, SplitSink<WebSocket, Message>>>,
    clients: Mutex<HashMap<u64, tokio::sync::mpsc::Sender<JsonRpcResponse>>>,
}

impl ConnectionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
        })
    }
}

// TODO fix the retunr values so they make sense
async fn handle_jsonrpc_request(
    auth: crate::middleware::auth::MyJWT,
    Path(endpoint_dongle_id): Path<String>,
    State(ctx): State<AppContext>,
    Extension(manager): Extension<Arc<ConnectionManager>>,
    Json(mut payload): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let user_model = users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
    let device_model = devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await?;
    if !user_model.superuser {
        enforce_ownership_rule!(
            user_model.id, 
            device_model.owner_id,
            "Can only communicate with your own device!"
        )
    }
    let now_id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    payload.id += now_id; // roll over is ok
    let message = Message::Text(serde_json::to_string(&payload).unwrap());

    if let Err(_e) = forward_command_to_device(&endpoint_dongle_id, &manager, &message).await {
        return loco_rs::controller::bad_request("Device not connected");
    }

    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<JsonRpcResponse>(1);
    {
        let mut clients = manager.clients.lock().await;
        clients.insert(now_id.clone(), response_tx);
    }

    loop {
        match time::timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(mut response)) => {
                response.id -= now_id;
                //let mut clients = manager.clients.lock().await;
                //clients.remove(&now_id);
                return format::json(response)
            },
            Ok(None) => {
                // Acknowledge and continue waiting for a valid response
                continue;
            },
            Err(_e) => {
                // Remove client on timeout
                //let mut clients = manager.clients.lock().await;
                //clients.remove(&now_id);
                return loco_rs::controller::bad_request("timed out");
            },
        }
    }
}

async fn forward_command_to_device(endpoint_dongle_id: &str, manager: &Arc<ConnectionManager>, message: &Message) -> Result<(), axum::Error> {
    let mut devices = manager.devices.lock().await;
    if let Some(device_sender) = devices.get_mut(endpoint_dongle_id) {
        device_sender.send(message.clone()).await.map_err(|_e| {
            axum::Error::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to send message to device"))
        })
    } else {
        tracing::trace!("No device found for client ID {}", endpoint_dongle_id);
        Err(axum::Error::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Device not found")))
    }
}

async fn exit_handler(
    ctx: &AppContext,
    endpoint_dongle_id: String,
    jwt_identity: String,
    manager: Arc<ConnectionManager>,
) {
    let _is_device = jwt_identity == endpoint_dongle_id;
    {
        tracing::debug!("Removing device to manager: {}", endpoint_dongle_id);
        let mut connections: tokio::sync::MutexGuard<HashMap<String, SplitSink<WebSocket, Message>>> = manager.devices.lock().await;
        connections.remove(&endpoint_dongle_id);
    } // unlock
    let device = match  devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await {
        Ok(device) => device,
        Err(_) => return,
    };
    let mut device_active_model = device.into_active_model();
    device_active_model.online = ActiveValue::Set(false);
    match device_active_model.update(&ctx.db).await {
        Ok(_) => (),
        Err(_e) => return,
    }
}

// async fn reset_dongle(endpoint_dongle_id: &str, manager: &Arc<ConnectionManager>) -> () {
//     let json_request = JsonRpcRequest {
//         method: "resetDongle".to_string(),
//         params: Some(serde_json::json!({})),
//         jsonrpc: "2.0".to_string(),
//         id: 1,
//     };
//     match serde_json::to_string(&json_request) {
//         Ok(json_message) => {
//             let ws_message = Message::Text(json_message);
//             tracing::info!("Device not registered. Reseting DongleId: {}", endpoint_dongle_id);
//             if let Err(e) = forward_command_to_device(&endpoint_dongle_id, &manager, &ws_message).await {
//                 tracing::error!("Failed to forward command to device: {}", e);
//             }
//         },
//         Err(e) => {
//             tracing::trace!("Failed to serialize JSON-RPC request: {}", e);
//         }
//     }
// }

async fn handle_socket(
    ctx: &AppContext,
    socket: WebSocket,
    endpoint_dongle_id: String,
    jwt_identity: String,
    manager: Arc<ConnectionManager>,
) {
    let is_device = jwt_identity == endpoint_dongle_id;
    let _is_registered = devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await.is_ok();

    let (sender, mut receiver) = socket.split();

    if is_device {
        let mut devices: tokio::sync::MutexGuard<HashMap<String, SplitSink<WebSocket, Message>>> = manager.devices.lock().await;
        tracing::info!("Adding device to manager: {}", endpoint_dongle_id);
        devices.insert(endpoint_dongle_id.clone(), sender);
    }
    
    while let Some(message_result) = receiver.next().await {
        
        let message = match message_result {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("Error receiving message: {:?}", e);
                continue;  // Skip this iteration and continue listening
            }
        };
        match message {
            Message::Ping(_) => {println!("Ping: {jwt_identity}")}
            Message::Pong(_) => {
                tracing::trace!("Pong: {jwt_identity}");
                // update last_athena_ping time here
                let device = match  devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await {
                    Ok(device) => device,
                    Err(_e) => break,
                };
                let mut device_active_model = device.into_active_model();
                device_active_model.last_athena_ping = ActiveValue::Set(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64);
                device_active_model.online = ActiveValue::Set(true);
                match device_active_model.update(&ctx.db).await {
                    Ok(_) => (),
                    Err(_e) => break,
                }
            }
            Message::Close(_) => {
                tracing::debug!("{} WebSocket Closed {endpoint_dongle_id}", if is_device { "Device" } else {"Client"} );
                break;
            }
            Message::Text(text) => {
                if let Ok(JsonRpcResponse { result, error, jsonrpc, id }) = serde_json::from_str(&text) {
                    let mut clients = manager.clients.lock().await;
                    if let Some(client_sender) = clients.remove(&id) {
                        let _ = client_sender.send(JsonRpcResponse { result, error, jsonrpc, id }).await;
                    }
                }
                // handle other things
            }
            Message::Binary(_bin) => {
                // let response: JsonRpcResponse = serde_json::from_str(&bin).unwrap();
                // let mut clients = manager.clients.lock().await;
                // if let Some(client_sender) = clients.remove(&endpoint_dongle_id) {
                //     let _ = client_sender.send(response).await;
                // }
            }
        }
    }
    tracing::trace!("Connection out of context.");
    exit_handler(ctx,endpoint_dongle_id, jwt_identity, manager).await;
}

async fn handle_device_ws(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    _headers: HeaderMap,
    ws: WebSocketUpgrade,
    axum::extract::Path(endpoint_dongle_id): axum::extract::Path<String>,
    Extension(manager): Extension<Arc<ConnectionManager>>,
) -> impl IntoResponse {
    if auth.device_model.is_none() { // if a user is trying to make a websocket connection they need to be device owner or superuser
        let user_model = users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
        let device_model = devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await?;
        if !user_model.superuser {
            enforce_ownership_rule!(
                user_model.id, 
                device_model.owner_id,
                "Can only communicate with your own device!"
            )
        }
    } else if auth.claims.identity != endpoint_dongle_id{ // if a device is trying to connect to another device
        tracing::error!("Someone is trying to make illegal access: from {} to {endpoint_dongle_id}", auth.claims.identity);
        return unauthorized("Devices shouldn't talk to eachother!");
    }
    Ok(ws.on_upgrade(move |socket| async move {
        handle_socket(&ctx, socket, endpoint_dongle_id, auth.claims.identity, manager).await;
    }))
}

pub async fn send_ping_to_all_devices(manager: Arc<ConnectionManager>) {
    let mut devices = manager.devices.lock().await;
    for (id, sender) in devices.iter_mut() {
        tracing::trace!("Sending ping to {}", &id);
        if let Err(e) = sender.send(Message::Ping(Vec::new())).await {
            tracing::trace!("Failed to send ping to device {}: {}", id, e);
        }
    }
}

pub async fn send_reset( // called from crate::middleware::auth
    _ctx: &AppContext,
    socket: WebSocket
) {
    let (mut sender, _receiver) = socket.split();
        let reset_dongle_rpc = JsonRpcRequest {
            method: "resetDongle".to_string(),
            params: Some(serde_json::json!({})),
            jsonrpc: "2.0".to_string(),
            id: 1,
        };
        let msg = serde_json::to_string(&reset_dongle_rpc).unwrap();
        match sender.send(Message::Text(msg)).await {
            Ok(_) => tracing::info!("Sent resetDongle"),
            Err(_) => tracing::error!("Failed to send resetDongle"),
        }
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("ws")
        .add("/v2/:dongle_id", get( handle_device_ws))
        .add("/:dongle_id", get( handle_device_ws))
        .add("/:dongle_id", post( handle_jsonrpc_request))
}
