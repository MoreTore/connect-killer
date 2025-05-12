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
use std::{collections::{HashMap}, sync::Arc};
use tokio::sync::{RwLock, Mutex};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{self, Duration};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

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

fn generate_request_id() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum Id {
    String(String),
    Int(u64),
}

impl From<Id> for String {
    fn from(id: Id) -> Self {
        match id {
            Id::String(s) => s,
            Id::Int(i) => i.to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct JsonRpcRequest {
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub jsonrpc: String,
    pub id: Id,
}


impl Default for JsonRpcRequest {
    fn default() -> Self {
        Self {
            method: "".to_string(),
            params: None,
            jsonrpc: "2.0".to_string(),
            id: Id::String(generate_request_id()),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct JsonRpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
    jsonrpc: String,
    id: Id,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
}

pub struct DeviceConnection {
    pub connection_id: String,
    pub sender: SplitSink<WebSocket, Message>,
}

pub struct ConnectionManager {
    pub devices: Mutex<HashMap<String, DeviceConnection>>,
    pub clients: Mutex<HashMap<String, tokio::sync::mpsc::Sender<JsonRpcResponse>>>,
    // branch -> module -> Vec<serde_json::Value>
    pub cloudlog_cache: RwLock<HashMap<String, HashMap<String, HashMap<String, Vec<serde_json::Value>>>>>,
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

async fn handle_jsonrpc_request(
    auth: crate::middleware::auth::MyJWT,
    Path(endpoint_dongle_id): Path<String>,
    State(ctx): State<AppContext>,
    Extension(manager): Extension<Arc<ConnectionManager>>,
    Json(mut payload): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let is_sensitive_method = payload.method == "takeSnapshot";
    if let Some(user_model) = auth.user_model {
        if !user_model.superuser || is_sensitive_method {
            DM::ensure_user_device(&ctx.db, user_model.id, &endpoint_dongle_id).await?;
        }
    } else {
        return Err(Error::Unauthorized("Devices can't do this".to_string()));
    }
    
    let now_id = generate_request_id();
    // Convert the current id into a String.
    let mut id_string: String = payload.id.into();
    // Append the generated now_id.
    id_string.push_str(&now_id);
    // Update payload.id with the new string.
    payload.id = Id::String(id_string.clone()); 
    
    let message = Message::Text(serde_json::to_string(&payload)?);
    forward_command_to_device(&endpoint_dongle_id, &manager, &message).await?;
    
    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<JsonRpcResponse>(1);
    {
        let mut clients = manager.clients.lock().await;
        clients.insert(id_string, response_tx);
    }
    
    loop {
        match time::timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(mut response)) => {
                // Convert response.id to a String for manipulation.
                let mut response_id_string: String = response.id.into();
                // If it ends with the appended now_id, remove that part.
                if response_id_string.ends_with(&now_id) {
                    let new_len = response_id_string.len() - now_id.len();
                    response_id_string.truncate(new_len);
                }
                // Update response.id with the new value.
                response.id = Id::String(response_id_string);
                return Ok(format::json(response));
            },
            Ok(None) => {
                // Acknowledge and continue waiting for a valid response.
                continue;
            },
            Err(_e) => {
                // Remove client on timeout.
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
    let mut device_connections = manager.devices.lock().await;
    let device_conn = device_connections.get_mut(endpoint_dongle_id).ok_or(Error::DeviceNotFound)?;

    device_conn
        .sender
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
    connection_id: String,
) {
    let is_device = jwt_identity == endpoint_dongle_id;
    {
        let mut device_connections = manager.devices.lock().await;

        // Check if the connection ID matches the one in the manager. In some cases, the device might have tried to reconnect before
        // the old connection was removed, so we need to ensure we are removing the correct one and not drop the new connection.
        if let Some(device_conn) = device_connections.get(&endpoint_dongle_id) {
            if device_conn.connection_id == connection_id {
                tracing::info!("Removing device from manager: {}", endpoint_dongle_id);
                device_connections.remove(&endpoint_dongle_id);
            }
        }

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
    let (mut sender, mut receiver) = socket.split();
    let connection_id = Uuid::new_v4().to_string();

    if is_device {
        let mut devices: tokio::sync::MutexGuard<HashMap<String, DeviceConnection>> = manager.devices.lock().await;
        tracing::info!("Adding device to manager: {}", endpoint_dongle_id);
        devices.insert(endpoint_dongle_id.clone(), DeviceConnection {
            connection_id: connection_id.clone(),
            sender,
        });
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
                if let Ok(message) = serde_json::from_str::<JsonRpcMessage>(&text) {
                    match message {
                        JsonRpcMessage::Response(resp) => {
                            let mut clients = manager.clients.lock().await;
                            let id_str: String = resp.id.clone().into();
                            if let Some(client_sender) = clients.remove(&id_str) {
                                let _ = client_sender.send(resp).await;
                            }
                        },
                        JsonRpcMessage::Request(req) => {
                            // Create a response based on the method.
                            let response = match req.method.as_str() {
                                "forwardLogs" => {
                                    if let Some(params) = req.params {
                                        if let Some(logs_str) = params.get("logs").and_then(|v| v.as_str()) {
                                            // Parse logs using a streaming deserializer.
                                            let mut parsed_logs = Vec::new();
                                            let mut had_parse_errors = false;
                                            let mut stream = serde_json::Deserializer::from_str(&logs_str)
                                                .into_iter::<serde_json::Value>();
                                            while let Some(result) = stream.next() {
                                                match result {
                                                    Ok(log) => parsed_logs.push(log),
                                                    Err(e) => {
                                                        tracing::error!("Error parsing log: {}", e);
                                                        had_parse_errors = true;
                                                    }
                                                }
                                            }
                            
                                            // Now store logs in a nested hashmap:
                                            {
                                                let mut cloudlog_cache = manager.cloudlog_cache.write().await;
                                                // Get or create the entry for this device.
                                                let device_logs = cloudlog_cache
                                                    .entry(endpoint_dongle_id.clone())
                                                    .or_insert_with(HashMap::new);
                            
                                                for log in parsed_logs {
                                                    // Extract branch from ctx. If missing, default to "unknown".
                                                    let branch = log.get("ctx")
                                                        .and_then(|ctx| ctx.get("branch"))
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("unknown")
                                                        .to_string();
                                                    // Extract module. If missing, default to "unknown".
                                                    let module = log.get("filename")
                                                        .and_then(|m| m.as_str())
                                                        .unwrap_or("unknown")
                                                        .to_string();
                                                    
                                                    // For this branch, get or create the module map.
                                                    let branch_map = device_logs.entry(branch).or_insert_with(HashMap::new);
                                                    // Get or create the vector for the module.
                                                    let module_logs = branch_map.entry(module).or_insert_with(Vec::new);
                                                    module_logs.insert(0, log);
                                                    module_logs.truncate(50); // limit to 50 of the latest logs
                                                }
                                            }
                            
                                            // Respond based on parse results.
                                            if had_parse_errors {
                                                JsonRpcResponse {
                                                    result: None,
                                                    error: Some(serde_json::json!({
                                                        "code": -32000,
                                                        "message": "Some log lines failed to parse"
                                                    })),
                                                    jsonrpc: "2.0".to_string(),
                                                    id: req.id,
                                                }
                                            } else {
                                                JsonRpcResponse {
                                                    result: Some(serde_json::json!({"status": "ok"})),
                                                    error: None,
                                                    jsonrpc: "2.0".to_string(),
                                                    id: req.id,
                                                }
                                            }
                                        } else {
                                            // "logs" parameter is missing.
                                            JsonRpcResponse {
                                                result: None,
                                                error: Some(serde_json::json!({
                                                    "code": -32602,
                                                    "message": "Missing 'logs' parameter"
                                                })),
                                                jsonrpc: "2.0".to_string(),
                                                id: req.id,
                                            }
                                        }
                                    } else {
                                        // Parameters missing.
                                        JsonRpcResponse {
                                            result: None,
                                            error: Some(serde_json::json!({
                                                "code": -32602,
                                                "message": "Missing parameters"
                                            })),
                                            jsonrpc: "2.0".to_string(),
                                            id: req.id,
                                        }
                                    }
                                },
                                _ => {
                                    // If the method is not recognized, send a method not found error.
                                    JsonRpcResponse {
                                        result: None,
                                        error: Some(serde_json::json!({
                                            "code": -32601,
                                            "message": format!("Method '{}' not found", req.method)
                                        })),
                                        jsonrpc: "2.0".to_string(),
                                        id: req.id,
                                    }
                                }
                            };
                            // Convert the response to a text message and forward it.
                            let message = Message::Text(serde_json::to_string(&response).unwrap());
                            let _ = forward_command_to_device(&endpoint_dongle_id, &manager, &message).await;
                        },
                        _ => {
                            tracing::error!("Invalid JSON-RPC message: {}", text);
                        }
                    }
                }
            }
            Message::Binary(_bin) => { // lots of cloudlogs coming in here. Maybe send it to the web interface client for real-time rollout monitoring
                tracing::info!("Got Binary Data from {}", endpoint_dongle_id);
            }
        }
    }
    tracing::trace!("Connection out of context.");
    exit_handler(ctx,endpoint_dongle_id, jwt_identity, manager, connection_id).await;
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
    for (dongle_id, device_connection) in devices.iter_mut() {
        tracing::trace!("Sending ping to {}", &dongle_id);
        if let Err(e) = device_connection.sender.send(Message::Ping(Vec::new())).await {
            tracing::trace!("Failed to send ping to device {}: {}", dongle_id, e);
        }
    }
    for (dongle_id, device_connection) in devices.iter_mut() {
        if let Ok(Some(latest_msg)) = DMQM::find_latest_msg(db, &dongle_id).await {
            if let Err(e) = device_connection.sender.send(Message::Text(latest_msg.json_rpc_request.to_string())).await {
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
            id: Id::Int(1),
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
