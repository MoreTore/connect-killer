use axum::{
    extract::{ws::{
        Message,
        WebSocket,
        WebSocketUpgrade},
        Path,
        FromRequest, 
        },
    http::HeaderMap,
    response::IntoResponse,
    routing::get, 
    Extension, 
    Router
};
use cookie::Cookie;
use futures::{stream::SplitStream, SinkExt, StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use serde_json::Value;
use loco_rs::app::AppContext;
use futures::stream::SplitSink;

struct DeviceConnections {
    connections: Mutex<HashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>>,
}

impl DeviceConnections {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
}

async fn extract_jwt_from_cookie(headers: &HeaderMap) -> Option<String> {
    // Check if the 'cookie' header is present in the request
    if let Some(cookie_header) = headers.get("cookie") {
        // Convert the cookie header to a string
        for cookie in Cookie::split_parse_encoded(cookie_header) {
            let cookie = cookie.unwrap();
            match cookie.name() {
                "name" => {return Some(cookie.value().into());}
                _ => return None,
            }
        }
    }
    None
}

async fn forward_to_device(
    devices: Arc<DeviceConnections>,
    jwt: String,
    dongle_id: String,
    message: Value,
) -> Result<(), String> {
    let connections = devices.connections.lock().await;
    if let Some(tx) = connections.get(&dongle_id) {
        let mut tx = tx.lock().await;  // Properly access the locked SplitSink
        let json_message = serde_json::to_string(&message).unwrap();
        if let Err(e) = tx.send(Message::Text(json_message)).await {
            return Err(format!("Failed to send message: {:?}", e));
        }
    } else {
        return Err("Device not connected".to_string());
    }
    Ok(())
}

async fn handle_ws(
    mut rx: SplitStream<WebSocket>, 
    ctx: AppContext, 
    jwt: String,
    dongle_id: String, 
    devices: Arc<DeviceConnections>) {
    
    let msg = rx.next().await;
    let m =  match msg {
        Some(msg) => msg,
        None => return (),
    };
    let r = match m {
        Ok(rx) => rx,
        Err(e) => {println!("{:?}", e); return ();}
    };
    match r {
        Message::Text(text) => {
            let parsed: Result<Value, _> = serde_json::from_str(&text);
            match parsed {
                Ok(json) => {
                    println!("Received response: {json}");
                    // Check if the JSON object has a "method" key
                    if let Some(method) = json.get("method") {
                        // Check if the method is a string and proceed
                        if method.is_string() {
                            println!("Method found: {}", method.as_str().unwrap());
                            // If valid, forward the JSON message
                            if let Err(e) = forward_to_device(devices.clone(), jwt, dongle_id.clone(), json).await {
                                eprintln!("Error forwarding message: {}", e);
                            };
                        } else {
                            println!("Method key found but is not a string");
                        }
                    } else {
                        println!("JSON object does not contain 'method' key");
                    }
                }
                Err(err) => {
                    println!("Error parsing JSON: {:?}", err);
                }
            }
        }
        Message::Close(_) => {println!("WebSocket closed.");}
        Message::Binary(bin) => {println!("Binary: {:?}", bin)}
        Message::Ping(ping) => {println!("Ping: {:?}", ping)}
        Message::Pong(pong) => {println!("Pong: {:?}", pong)}
    }
}

async fn handle_device_ws(
    ctx: AppContext,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    dongle_id: String,
    devices: Arc<DeviceConnections>
) -> impl IntoResponse {
    let jwt = extract_jwt_from_cookie(&headers).await.unwrap_or_default();
    ws.on_upgrade(move |socket| async move {
        let (tx, rx) = socket.split();
        let tx = Arc::new(Mutex::new(tx));
        devices.connections.lock().await.insert(dongle_id.clone(), tx);
        handle_ws(rx, ctx, jwt, dongle_id, devices).await;
    })
}

pub fn ws_routes(
    ctx: AppContext
) -> Router {
    let devices = Arc::new(DeviceConnections::new());
    Router::new()
        .route("/ws/v2/:dongleid", get(move |headers: HeaderMap, ws: WebSocketUpgrade, Path(dongle_id): Path<String>| {
            handle_device_ws(ctx, headers, ws, dongle_id, devices.clone())
        }))
}