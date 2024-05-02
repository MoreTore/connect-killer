use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
 };
 use futures::stream::SplitSink;
use futures_util::{StreamExt, SinkExt};
 use serde_json::{json, Value};
 use std::pin::Pin;
 use tracing::error;
 
 async fn ws_send(tx: &mut Pin<Box<SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>) {
    let data = json!({
        "jsonrpc": "2.0",
        "method": "getPublicKey",
        "params": {},
        "id": 1,
    });
    tx.send(Message::Text(data.to_string())).await.unwrap();
 }
 
 async fn handle_ws(ws: WebSocket) {
    println!("Sending Command");
    let (tx, mut rx) = ws.split();
 
    // Pin `tx` as a boxed value
    let mut tx = Box::pin(tx);
 
    // Send initial command
    ws_send(&mut tx).await;
 
    while let Some(Ok(msg)) = rx.next().await {
        match msg {
            Message::Text(text) => {
                let parsed: Result<Value, _> = serde_json::from_str(&text);
                match parsed {
                    Ok(json) => {
                        handle_command(json, &mut tx).await;
                    }
                    Err(err) => {
                        error!("Error parsing JSON: {:?}", err);
                    }
                }
            }
            Message::Close(_) => {
                error!("WebSocket closed.");
                break;
            }
            _ => {}
        }
    }
 }
 
 async fn handle_command(command: Value, tx: &mut Pin<Box<SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>) {
    println!("Received response: {}", serde_json::to_string(&command).unwrap());

    let response = json!({ "status": "ok" });
    tx.send(Message::Text(response.to_string())).await.unwrap();
 }
 
 
pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    println!("here");
    ws.on_upgrade(handle_ws)
}

pub async fn ws_routes() -> axum::Router {
    axum::Router::new()
        .route("/ws/v2/:dongleid", axum::routing::get(ws_handler))
}