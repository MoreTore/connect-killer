#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use axum::{
    extract::{ws::{Message, 
                   WebSocket, 
                   WebSocketUpgrade}, Path 
             }, http::HeaderMap, response::IntoResponse, routing::get, Extension, Router 
};
use futures::stream::SplitSink;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use futures_util::{SinkExt, StreamExt};

use tokio::time::{self, Duration};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{middleware::identity, models::_entities::devices};

pub struct ConnectionManager {
    devices: Mutex<HashMap<String, futures::stream::SplitSink<WebSocket, Message>>>,
    clients: Mutex<HashMap<String, futures::stream::SplitSink<WebSocket, Message>>>,
}

impl ConnectionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
        })
    }
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
        Ok(device) => device,
        Err(_) => return,
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
    let is_registered = devices::Model::find_device(&ctx.db, &endpoint_dongle_id).await.is_ok();
    
    if !is_registered {
        tracing::info!("Got athena request from unregistered device: {}", endpoint_dongle_id);
        return
    } else {
        //Verify socket
    }

    let (sender, mut receiver) = socket.split();

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
                                    Ok(device) => device,
                                    Err(e) => break,
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
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    axum::extract::Path(endpoint_dongle_id): axum::extract::Path<String>,
    Extension(manager): Extension<Arc<ConnectionManager>>,
) -> impl IntoResponse {
    let jwt: String = identity::extract_jwt_from_cookie(&headers).await.unwrap_or_default();
    ws.on_upgrade(move |socket| async move {
        let jwt_identity = match identity::verify_identity(&ctx, &jwt).await {
            Ok(jwt_payload) => jwt_payload.identity,
            Err(err) => {
                tracing::debug!("Error verifying token: {:?}", err);
                "".into()
            }
        };
        handle_socket(&ctx, socket, endpoint_dongle_id, jwt_identity, manager).await;
    })
}

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("ws")
        .add("/", get(hello))
        .add("/echo", post(echo))
        .add("/v2/:dongle_id", get( handle_device_ws))
}
