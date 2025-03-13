#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{
        ws::{Message, 
                   WebSocket, 
                   WebSocketUpgrade}, Path 
        }, 
        http::HeaderMap, 
        response::IntoResponse, 
        routing::get, 
        Extension 
};
use futures::stream::SplitSink;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use std::{collections::{HashMap, VecDeque}, sync::Arc};
use tokio::sync::{RwLock, Mutex};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{self, Duration};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::{
    models::{
        _entities,
        devices::DM,
        device_msg_queues::DMQM,
    },
};


#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("device not found")]
    DeviceNotFound,
    #[error("failed to send message to device: {0}")]
    SendFailed(String),
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("websocket error: {0}")]
    WebSocketError(#[from] axum::Error),
    #[error("timeout")]
    Timeout,
    #[error("unauthorized: {0}")]
    Unauthorized(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::DeviceNotFound => (axum::http::StatusCode::NOT_FOUND, "Device not found").into_response(),
            Error::Timeout => (axum::http::StatusCode::REQUEST_TIMEOUT, "Timed out").into_response(),
            Error::Unauthorized(msg) => (axum::http::StatusCode::UNAUTHORIZED, msg).into_response(),
            _ => {
                tracing::error!("Unhandled error: {:?}", self);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                )
                    .into_response()
            }
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;

fn generate_request_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[derive(Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub jsonrpc: String,
    pub id: u64,
}


impl Default for JsonRpcRequest {
    fn default() -> Self {
        Self {
            method: "".to_string(),
            params: None,
            jsonrpc: "2.0".to_string(),
            id: generate_request_id(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct JsonRpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    jsonrpc: String,
    id: u64,
}

pub struct ConnectionManager {
    pub devices: Mutex<HashMap<String, SplitSink<WebSocket, Message>>>,
    pub clients: Mutex<HashMap<u64, tokio::sync::mpsc::Sender<JsonRpcResponse>>>,
    pub cloudlog_cache: RwLock<HashMap<String, VecDeque<u8>>>,
}

impl ConnectionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
            cloudlog_cache: RwLock::new(HashMap::new()),
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
) -> Result<impl IntoResponse> {
    let is_sensitive_method = payload.method == "takeSnapshot".to_string();
    if let Some(user_model) = auth.user_model {
        if !user_model.superuser || is_sensitive_method {
            DM::ensure_user_device(&ctx.db, user_model.id, &endpoint_dongle_id).await?;
        }
    } else {
        return Err(Error::Unauthorized("Devices can't do this".to_string()));
    }
    
    let now_id = generate_request_id();
    payload.id += now_id; // roll over is ok
    let message = Message::Text(serde_json::to_string(&payload)?);

    forward_command_to_device(&endpoint_dongle_id, &manager, &message).await?;

    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<JsonRpcResponse>(1);
    {
        let mut clients = manager.clients.lock().await;
        clients.insert(now_id.clone(), response_tx);
    }

    loop {
        match time::timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(mut response)) => {
                response.id -= now_id;
                return Ok(format::json(response));
            },
            Ok(None) => {
                // Acknowledge and continue waiting for a valid response
                continue;
            },
            Err(_e) => {
                // Remove client on timeout
                let mut clients = manager.clients.lock().await;
                clients.remove(&now_id);
                return Err(Error::Timeout);
            },
        }
    }
}

async fn forward_command_to_device(
    endpoint_dongle_id: &str,
    manager: &Arc<ConnectionManager>,
    message: &Message,
) -> Result<()> {
    let mut devices = manager.devices.lock().await;
    let device_sender = devices.get_mut(endpoint_dongle_id).ok_or(Error::DeviceNotFound)?;

    device_sender
        .send(message.clone())
        .await
        .map_err(|e| Error::SendFailed(e.to_string()))?;
    Ok(())
}

async fn exit_handler(
    ctx: &AppContext,
    endpoint_dongle_id: String,
    jwt_identity: String,
    manager: Arc<ConnectionManager>,
) {
    let is_device = jwt_identity == endpoint_dongle_id;
    {
        tracing::debug!("Removing device from manager: {}", endpoint_dongle_id);
        let mut connections = manager.devices.lock().await;
        connections.remove(&endpoint_dongle_id);
    } // unlock the mutex
    if is_device {
        if let Ok(device) = DM::find_device(&ctx.db, &endpoint_dongle_id).await {
            let mut device_active_model = device.into_active_model();
            device_active_model.online = ActiveValue::Set(false);
            if let Err(e) = device_active_model.update(&ctx.db).await {
                tracing::error!("Failed to update device status: {:?}", e);
            }
        }
    }
}

async fn handle_socket(
    ctx: &AppContext,
    socket: WebSocket,
    endpoint_dongle_id: String,
    jwt_identity: String,
    manager: Arc<ConnectionManager>,
) {
    let is_device = jwt_identity == endpoint_dongle_id;
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
                let device = match  DM::find_device(&ctx.db, &endpoint_dongle_id).await {
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
            Message::Binary(bin) => { // lots of cloudlogs coming in here. Maybe send it to the web interface client for real-time rollout monitoring
                let mut cloudlog_cache = manager.cloudlog_cache.write().await;
                tracing::info!("Appending binary data for {}", endpoint_dongle_id);
                // Get or create entry in cloudlog_cache
                let entry = cloudlog_cache.entry(endpoint_dongle_id.clone()).or_insert(VecDeque::new());
                // Append new data
                entry.extend(bin);
                // Ensure we don’t exceed 50 entries
                while entry.len() > 50 {
                    entry.pop_front(); // Remove oldest data
                }
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
    if let Some(user_model) = auth.user_model {
        if !user_model.superuser {
            DM::ensure_user_device(&ctx.db, user_model.id, &endpoint_dongle_id).await?;
        }
    } else if auth.claims.identity != endpoint_dongle_id{ // if a device is trying to connect to another device
        tracing::error!("Someone is trying to make illegal access: from {} to {endpoint_dongle_id}", auth.claims.identity);
        return unauthorized("Devices shouldn't talk to eachother!");
    }
    Ok(ws.on_upgrade(move |socket| async move {
        handle_socket(&ctx, socket, endpoint_dongle_id, auth.claims.identity, manager).await;
    }))
}

pub async fn send_ping_to_all_devices(manager: Arc<ConnectionManager>, db: &DatabaseConnection) {
    let mut devices = manager.devices.lock().await;
    for (dongle_id, sender) in devices.iter_mut() {
        tracing::trace!("Sending ping to {}", &dongle_id);
        if let Err(e) = sender.send(Message::Ping(Vec::new())).await {
            tracing::trace!("Failed to send ping to device {}: {}", dongle_id, e);
        }
    }
    for (dongle_id, sender) in devices.iter_mut() {
        if let Ok(Some(latest_msg)) = DMQM::find_latest_msg(db, &dongle_id).await {
            if let Err(e) = sender.send(Message::Text(latest_msg.json_rpc_request.to_string())).await {
                tracing::error!("Failed to send jsonrpc msg to device {}: {}", dongle_id, e);
            }
            if _entities::device_msg_queues::Entity::delete_by_id(latest_msg.id).exec(db).await.is_err() {
                tracing::error!("Failed to delete msg in queue {}", dongle_id);
            }
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
