use axum::{
    http::HeaderMap, 
    response::IntoResponse, 
    routing::get, 
    Router,
    extract::{Path,
              ws::{Message, 
                   WebSocket, 
                   WebSocketUpgrade}, 
             }, 
};
use futures::stream::SplitSink;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use serde::{Deserialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use futures_util::{SinkExt, StreamExt};

use tokio::time::{self, Duration};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::websockets::identity;
use crate::models::_entities::{devices,};

struct ConnectionManager {
    devices: Mutex<HashMap<String, SplitSink<WebSocket, Message>>>,
    clients: Mutex<HashMap<String, SplitSink<WebSocket, Message>>>,
}

impl ConnectionManager {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
        })
    }
}

struct DeviceResponse {
    result: serde_json::Value,
}

async fn forward_message_to_client(endpoint_dongle_id: &str, manager: &Arc<ConnectionManager>, message: &Message) -> Result<(), axum::Error> {
    let mut clients = manager.clients.lock().await;
    if let Some(client_sender) = clients.get_mut(endpoint_dongle_id) {
        client_sender.send(message.clone()).await.map_err(|_| {
            axum::Error::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to send message to client"))
        })
    } else {
        tracing::trace!("No client found for device ID {}", endpoint_dongle_id);
        Err(axum::Error::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Client not found")))
    }
}

async fn forward_command_to_device(endpoint_dongle_id: &str, manager: &Arc<ConnectionManager>, message: &Message) -> Result<(), axum::Error> {
    let mut devices = manager.devices.lock().await;
    if let Some(device_sender) = devices.get_mut(endpoint_dongle_id) {
        device_sender.send(message.clone()).await.map_err(|e| {
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
    let is_device = jwt_identity == endpoint_dongle_id;
    {
        let mut connections: tokio::sync::MutexGuard<HashMap<String, SplitSink<WebSocket, Message>>> = if is_device {
            tracing::info!("Adding device to manager: {}", endpoint_dongle_id);
            manager.devices.lock().await
        } else {
            tracing::info!("Adding client to manager: {}", endpoint_dongle_id);
            manager.clients.lock().await
        };
        connections.remove(&endpoint_dongle_id);
    } // unlock
    let device = match  devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await {
        Some(device) => device,
        None => return,
    };
    let mut device_active_model = device.into_active_model();
    device_active_model.online = ActiveValue::Set(false);
    match device_active_model.update(&ctx.db).await {
        Ok(_) => (),
        Err(e) => return,
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
    let is_registered = devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await.is_some();
    if !is_registered {
        tracing::info!("Got athena request from unregistered device: {}", endpoint_dongle_id);
        return
    } else {
        //Verify socket
    }

    let (mut sender, mut receiver) = socket.split();

    {
        let mut connections: tokio::sync::MutexGuard<HashMap<String, SplitSink<WebSocket, Message>>> = if is_device {
            tracing::info!("Adding device to manager: {}", endpoint_dongle_id);
            manager.devices.lock().await
        } else {
            tracing::info!("Adding client to manager: {}", endpoint_dongle_id);
            manager.clients.lock().await
        };
        connections.insert(endpoint_dongle_id.clone(), sender);
    } // unlock

    

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
            Message::Pong(_) => {tracing::trace!("Pong: {jwt_identity}");
                                // update last_ping time here
                                let device = match  devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await {
                                    Some(device) => device,
                                    None => break,
                                };
                                let mut device_active_model = device.into_active_model();
                                device_active_model.last_ping = ActiveValue::Set(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64);
                                device_active_model.online = ActiveValue::Set(true);
                                match device_active_model.update(&ctx.db).await {
                                    Ok(_) => (),
                                    Err(e) => break,
                                }
                                }
            Message::Close(_) => {
                tracing::debug!("{} WebSocket Closed {endpoint_dongle_id}", if is_device { "Device" } else {"Client"} );
                break;
            }
            _ => {
                // forward between client and device
                let result = if is_device {
                    forward_message_to_client(&endpoint_dongle_id, &manager, &message).await
                } else {
                    forward_command_to_device(&endpoint_dongle_id, &manager, &message).await
                };
                // Check the result of the send operation
                if let Err(e) = result {
                    tracing::debug!("Failed to send message: {}", e);
                }
            }
        }
    }
    tracing::trace!("Connection out of context.");
    exit_handler(ctx,endpoint_dongle_id, jwt_identity, manager).await;
}

async fn handle_device_ws(
    ctx: AppContext,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    endpoint_dongle_id: String,
    manager: Arc<ConnectionManager>
) -> impl IntoResponse {
    let jwt: String = identity::extract_jwt_from_cookie(&headers).await.unwrap_or_default();
    ws.on_upgrade(move |socket: WebSocket| async move {

        let jwt_identity = match identity::decode_jwt_identity(&jwt) {
            Ok(token_data) => token_data.identity,
            Err(err) => {
                tracing::debug!("Error decoding JWT: {:?}", err);
                "".into()
            }
        };
        handle_socket(&ctx, socket, endpoint_dongle_id, jwt_identity, manager).await;
    })
}

async fn send_ping_to_all_devices(manager: Arc<ConnectionManager>) {
    let mut devices = manager.devices.lock().await;
    for (id, sender) in devices.iter_mut() {
        tracing::trace!("Sending ping to {}", &id);
        if let Err(e) = sender.send(Message::Ping(Vec::new())).await {
            tracing::trace!("Failed to send ping to device {}: {}", id, e);

        }
    }
}


pub fn ws_routes(
    ctx: AppContext
) -> Router {
    let manager: Arc<ConnectionManager> = ConnectionManager::new();
    let ping_manager = manager.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30)); // Ping every 30 seconds
        loop {
            interval.tick().await;
            send_ping_to_all_devices(ping_manager.clone()).await;
        }
    });
    Router::new()
        .route("/ws/v2/:dongleid", get(move |headers: HeaderMap, ws: WebSocketUpgrade, Path(endpoint_dongle_id): Path<String>| {
            handle_device_ws(ctx, headers, ws, endpoint_dongle_id, manager.clone())
        }))
}