#![allow(clippy::unused_async)]
use futures::FutureExt;
use loco_rs::prelude::*;
use reqwest::Client;
use serde_json::json;
use crate::views;
use crate::models::segments::{SegmentParams, segments};
use serde::{Deserialize, Serialize};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State}, 
    http::response, Extension,
    http::{HeaderMap, StatusCode},
};
extern crate url;
use url::form_urlencoded;

#[derive(Deserialize)]
struct UlogQuery {
    url: String
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String
}

#[derive(Deserialize)]
struct OneBox {
    onebox: String
}

#[derive(Serialize)]
pub struct SegmentsTemplate {
    pub segments: Vec<segments::Model>,
}

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}
pub async fn render_route(ViewEngine(v): ViewEngine<TeraView>, State(ctx): State<AppContext>) -> Result<impl IntoResponse> {
    let mut segs = segments::Model::find_all_segments(&ctx.db).await?;
    let route = SegmentsTemplate { segments: segs };
    views::route::admin_route(v, route)
}

pub async fn render_segment_ulog(
    ViewEngine(v): ViewEngine<TeraView>, 
    State(ctx): State<AppContext>,
    Extension(client): Extension<Client>,
    Query(mut params): Query<UlogQuery>
) -> Result<impl IntoResponse> {
    let request = client.get(params.url);
    // get the data and save it as a string and pass to admin_segment_ulog
    let res = request.send().await;
    let mut data: String;
    match res {
        Ok(response) => {
            let bytes = response.bytes().await.unwrap();
            let bytes_vec: Vec<u8> = bytes.to_vec(); // Convert &bytes::Bytes to Vec<u8>
            data = unsafe { String::from_utf8_unchecked(bytes_vec) };
        }
        _ => data = "No parsed data for this segment".to_string(),
    }

    views::route::admin_segment_ulog(v, UlogText { text: data })

}

pub async fn onebox_handler(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<Client>,
    Query(mut params): Query<OneBox>
) -> impl IntoResponse {
    // Decode the URL
    let search: Vec<&str>= params.onebox.split("%2F").collect();
    // get the size

}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("useradmin")
        .add("/", get(render_route))
        .add("/logs/", get(render_segment_ulog))
        .add("/echo", post(echo))
}
