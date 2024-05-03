use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::stream::SplitSink;
use futures_util::{StreamExt, SinkExt};
use loco_rs::app::AppContext;
use serde_json::{json, Value};
use std::pin::Pin;
use tracing::error;


fn initialize_requests() -> Vec<Value> {
    vec![
        json!({
            "jsonrpc": "2.0",
            "method": "getPublicKey",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "takeSnapshot",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getNetworks",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getNetworkMetered",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getNetworkType",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getSimInfo",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getGithubUsername",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getSshAuthorizedKeys",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "listUploadQueue",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "listDataDirectory",
            "params": {},
            "id": 1,
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "getVersion",
            "params": {},
            "id": 1,
        }),
    ]
}

async fn ws_send(tx: &mut Pin<Box<SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>) {
    let requests = initialize_requests();
    for req in requests {
        tx.send(Message::Text(req.to_string())).await.unwrap();
    }
}
 
async fn handle_ws(ws: WebSocket, ctx: AppContext) {
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
                        println!("Received response: {}", serde_json::to_string(&json).unwrap());
                        //handle_command(json, &mut tx).await;
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
            Message::Binary(bin) => {println!("Binary: {:?}", bin)}
            Message::Ping(ping) => {println!("Ping: {:?}", ping)}
            Message::Pong(pong) => {println!("Pong: {:?}", pong)}
        }
    }
}
 
async fn handle_command(command: Value, tx: &mut Pin<Box<SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>) {
    

    let response = json!({ "status": "ok" });
    
    //tx.send(Message::Text(response.to_string())).await.unwrap();
}
 
 
pub async fn ws_handler(ws: WebSocketUpgrade, ctx: AppContext) -> impl IntoResponse {
    ws.on_upgrade(move |ws| handle_ws(ws, ctx))
}

pub async fn ws_routes(ctx: AppContext) -> axum::Router {
    axum::Router::new()
        .route("/ws/v2/:dongleid", axum::routing::get(move |ws| ws_handler(ws, ctx.clone())))
}