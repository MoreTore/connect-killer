#![allow(clippy::unused_async)]
use futures::FutureExt;
use loco_rs::prelude::*;
use reqwest::Client;
use crate::models::_entities;

use crate::views;
use serde::{Deserialize, Serialize};
use axum::{
    extract::{Query, State}, Extension,
};
extern crate url;

#[derive(Deserialize)]
struct UlogQuery {
    url: String
}

#[derive(Serialize)]
pub struct UlogText {
   pub text: String
}

#[derive(Deserialize)]
pub struct OneBox {
    onebox: String
}

#[derive(Serialize)]
pub struct RoutesTemplate {
    pub defined: bool,
    pub routes: Vec<_entities::routes::Model>
}
#[derive(Serialize)]
pub struct DevicesTemplate {
    pub defined: bool,
    pub devices: Vec<_entities::devices::Model>
}
#[derive(Serialize)]
pub struct BootlogsTemplate {
    pub defined: bool,
    pub bootlogs: Vec<_entities::bootlogs::Model>
}

#[derive(Serialize)]
pub struct SegmentsTemplate {
    pub defined: bool,
    pub segments: Vec<_entities::segments::Model>,
}


#[derive(Serialize, Default)]
pub struct MasterTemplate {
    pub onebox: String,
    pub dongle_id: String,
    pub segments: Option<SegmentsTemplate>,
    pub devices: Option<DevicesTemplate>,
    pub routes: Option<RoutesTemplate>,
    pub bootlogs: Option<BootlogsTemplate>,
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

// pub async fn render_route_segments(
//     v: TeraView, 
//     ctx: AppContext,
//     canonical_route_name: String
// ) -> Result<impl IntoResponse> {
//     let mut segs = segments::Model::find_segments_by_route(&ctx.db, &canonical_route_name).await?;
//     let route = SegmentsTemplate { segments: segs, onebox: canonical_route_name, ..Default::default()};
//     views::route::admin_route(v, route)
// }

// pub async fn render_all_routes(
//     v: TeraView,
//     ctx: AppContext,
//     onebox: String
// ) -> Result<impl IntoResponse> {
//     let mut segs = segments::Model::find_all_segments(&ctx.db).await?;
//     // sort from old to new based on seg.start_time_utc_millis
//     segs.sort_by(|a, b| a.start_time_utc_millis.cmp(&b.start_time_utc_millis));
//     for seg in &segs {
//         println!("{}",seg.start_time_utc_millis);
//     }
//     let route = SegmentsTemplate { segments: segs, onebox: onebox, ..Default::default()};
//     views::route::admin_route(v, route)
// }

pub async fn render_segment_ulog(
    ViewEngine(v): ViewEngine<TeraView>, 
    State(ctx): State<AppContext>,
    Extension(client): Extension<Client>,
    Query(params): Query<UlogQuery>
) -> Result<impl IntoResponse> {
    let request = client.get(params.url);
    // get the data and save it as a string and pass to admin_segment_ulog
    let res = request.send().await;
    let data: String;
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
// pub async fn onebox_handler_old(
//     ViewEngine(v): ViewEngine<TeraView>,
//     State(ctx): State<AppContext>,
//     Extension(client): Extension<Client>,
//     Query(mut params): Query<OneBox>,
// ) -> Result<impl IntoResponse> {
//     let onebox = params.onebox.replace('/', "|");
//     let mut segs: Option<SegmentsTemplate> = None;
//     let mut routes: Option<RoutesTemplate> = None;
//     println!("{onebox}");
    
//     // validate 
//     if onebox.as_str() == "all" {
//         segs = Some(SegmentsTemplate { defined: true, segments: _entities::segments::Model::find_all_segments(&ctx.db).await? });
//     } else if onebox.contains('|') {
        
//         segs = Some(SegmentsTemplate { defined: true, segments: _entities::segments::Model::find_segments_by_route(&ctx.db, &onebox).await? });
//     } else { // development only
//         //segs = Some(SegmentsTemplate { defined: true, segments: segments::Model::find_all_segments(&ctx.db).await? });
//         if let Some(ref mut segs_template) = segs {
//             segs_template.segments.sort_by(|a, b| a.start_time_utc_millis.cmp(&b.start_time_utc_millis));
//         }        
//         routes = Some(RoutesTemplate { defined: true, routes: _entities::routes::Model::find_device_routes(&ctx.db, &onebox).await? });
//     }
//     let route = MasterTemplate { segments: segs, onebox: onebox, routes: routes, ..Default::default()};
//     views::route::admin_route(v, route)
// }

pub async fn onebox_handler(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    Extension(client): Extension<Client>,
    Query(params): Query<OneBox>,
) -> Result<impl IntoResponse> {
    // Regex to match a complete canonical route name
    let re = regex::Regex::new(r"^([0-9a-z]{16})([_|/|]?([0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2}))?$").unwrap();

    let mut canonical_route_name: Option<String> = None;
    let mut dongle_id: Option<String> = None;
    let mut timestamp: Option<String> = None;


    // Check for route or dongle ID
    if let Some(caps) = re.captures(&params.onebox) {
        dongle_id = Some(caps[1].to_string());
        if let Some(ts) = caps.get(3) {
            timestamp = Some(ts.as_str().to_string());
            canonical_route_name = Some(format!("{}|{}", dongle_id.as_ref().unwrap(), timestamp.as_ref().unwrap()));
        }
    }
    ctx.config;
    // Display diagnostic info
    println!("{:?}", canonical_route_name);
    println!("{:?}", dongle_id);
    println!("{:?}", timestamp);
    let mut segment_models = None;
    if let Some(canonical_route) = canonical_route_name {
        segment_models = Some(_entities::segments::Model::find_segments_by_route(&ctx.db, &canonical_route).await?);
        if let Some(segment_models) = segment_models.as_mut() {
            segment_models.sort_by(|a, b| a.number.cmp(&b.number));
        }
    
        // Create and render master template
        let master_template = MasterTemplate { 
            dongle_id: dongle_id.unwrap_or_default(),
            segments: segment_models.map(|segments| SegmentsTemplate { 
                defined: true, 
                segments 
            }), 
            onebox: params.onebox, 
            ..Default::default()
        };
    
        views::route::admin_route(v, master_template)
    } else if let Some(d_id) = dongle_id {
        let route_models = _entities::routes::Model::find_device_routes(&ctx.db, &d_id).await?;
        //let user = _entities::users::Model::find_by_pid(&ctx.db, &auth.claims.pid).await?;
        let device_models = _entities::devices::Model::find_all_devices(&ctx.db).await;

        let bootlogs_models = _entities::bootlogs::Model::find_device_bootlogs(&ctx.db, &d_id).await?;

        let master_template = MasterTemplate { 
            routes: Some(RoutesTemplate { 
                defined: true, 
                routes: route_models 
            }), 

            devices: Some(DevicesTemplate {
                defined: true,
                devices: device_models
            }),

            bootlogs: Some(BootlogsTemplate {
                defined: true,
                bootlogs: bootlogs_models
            }),

            onebox: params.onebox, 
            ..Default::default() };

        views::route::admin_route(v, master_template)

    } else {
        // Fallback response
        views::route::admin_route(v, MasterTemplate { onebox: params.onebox, ..Default::default() })
    }
}

pub async fn login(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse> {
    views::auth::login(v)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("useradmin")
        .add("/", get(onebox_handler))
        .add("/login", get(login))
        .add("/logs/", get(render_segment_ulog))
        .add("/echo", post(echo))
}
