#![allow(clippy::unused_async)]
use futures::FutureExt;
use loco_rs::prelude::*;
use reqwest::Client;
use serde_json::json;
use crate::models::_entities::routes;
use crate::models::devices;
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

#[derive(Serialize, Default)]
pub struct SegmentsTemplate {
    pub onebox: String,
    pub segments: Vec<segments::Model>,
    pub devices: Vec<devices::Model>,
    pub routes: Option<Vec<routes::Model>>,
    pub bootlogs: Vec<segments::Model>,
}

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}

// pub async fn render_user_devices(
//     ViewEngine(v): ViewEngine<TeraView>, 
//     State(ctx): State<AppContext>

// ) -> Result<impl IntoResponse> {
//     let mut segs = routes::Model::find_user_devices(&ctx.db).await?;
//     let route = SegmentsTemplate { segments: segs };
//     views::route::admin_route(v, route)
// }

// pub async fn render_device_routes(
//     ViewEngine(v): ViewEngine<TeraView>, 
//     State(ctx): State<AppContext>

// ) -> Result<impl IntoResponse> {
//     let mut segs = routes::Model::find_device_routes(&ctx.db).await?;
//     let route = SegmentsTemplate { segments: segs };
//     views::route::admin_route(v, route)
// }

pub async fn render_route_segments(
    v: TeraView, 
    ctx: AppContext,
    canonical_route_name: String
) -> Result<impl IntoResponse> {
    let mut segs = segments::Model::find_segments_by_route(&ctx.db, &canonical_route_name).await?;
    let route = SegmentsTemplate { segments: segs, onebox: canonical_route_name, ..Default::default()};
    views::route::admin_route(v, route)
}

pub async fn render_all_routes(
    v: TeraView,
    ctx: AppContext,
    onebox: String
) -> Result<impl IntoResponse> {
    let mut segs = segments::Model::find_all_segments(&ctx.db).await?;
    let route = SegmentsTemplate { segments: segs, onebox: onebox, ..Default::default()};
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


/// ?onebox=406f02914de1a867/2024-02-05--16-22-28 <- example of a specific route
/// route to render_route_segments(v,ctx,client,"406f02914de1a867_2024-02-05--16-22-28")
/// 
/// ?onebox=406f02914de1a867 <- example of a specific dongle
/// route to render_device_routes(v,ctx,client,"406f02914de1a867")
///
/// ?onebox=github_104254025 <- example of a specific user (github user id)
/// route to render_user_devices(v,ctx,client,"github_104254025")
/// 
/// ?onebox=all <- special case for all segments
/// route to render_all_routes(v,ctx)
/// route to the correct view based on the onebox query
pub async fn onebox_handler(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<Client>,
    Query(mut params): Query<OneBox>,
) -> Result<impl IntoResponse> {
    // Extract and normalize the "onebox" parameter
    let onebox = params.onebox.replace('/', "|");
    let mut segs;
    let mut routes;
    routes = None;
    if onebox.as_str() == "all" {
        segs = segments::Model::find_all_segments(&ctx.db).await?;
    } else if onebox.contains('|') {
        segs = segments::Model::find_segments_by_route(&ctx.db, &onebox).await?;
    } else {
        segs = segments::Model::find_all_segments(&ctx.db).await?;
        routes = Some(routes::Model::find_device_routes(&ctx.db, &onebox).await?);
    }
    let route = SegmentsTemplate { segments: segs, onebox: onebox, routes: routes, ..Default::default()};
    views::route::admin_route(v, route)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("useradmin")
        .add("/", get(onebox_handler))
        .add("/logs/", get(render_segment_ulog))
        .add("/echo", post(echo))
}
