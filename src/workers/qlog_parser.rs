use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use capnp::message::ReaderOptions;
use migration::m20240424_000004_segments::Segments as SegmentFields;
use crate::cereal::legacy_capnp::nav_update::segment;
use crate::models::_entities;
use crate::models::segments::{Model, ActiveModel};
use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter, Cursor};
use std::path::Path;
use anyhow::{Result, Context};
//use bzip2::read::BzDecoder;
use crate::cereal::log_capnp::{self, init_data};
use reqwest::{Body ,Client, Response};
use crate::common;

use std::time::Instant;
use crate::models::{ segments::SegmentParams, _entities::segments, _entities::routes, routes::RouteParams};
                
use futures::stream::TryStreamExt; // for stream::TryStreamExt to use try_next
use tokio_util::io::StreamReader;
use async_compression::tokio::bufread::BzDecoder;
use tokio::io::AsyncReadExt; // for read_to_end


pub struct LogSegmentWorker {
    pub ctx: AppContext,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Any(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type MyResult<T> = std::result::Result<T, Error>;

#[derive(Deserialize, Debug, Serialize)]
pub struct LogSegmentWorkerArgs {
    pub internal_file_url: String,
    pub dongle_id        : String,
    pub timestamp        : String,
    pub segment          : String,
    pub file             : String,
    pub create_time      : u64, // This is the time the call was made to the worker.
}

impl worker::AppWorker<LogSegmentWorkerArgs> for LogSegmentWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

#[async_trait]
impl worker::Worker<LogSegmentWorkerArgs> for LogSegmentWorker {
    async fn perform(&self, args: LogSegmentWorkerArgs) -> worker::Result<()> {
        let start = Instant::now();
        tracing::trace!("Starting QlogParser for URL: {}", args.internal_file_url);
        let client = Client::new();
        
        // Make sure we have the data in the key value store
        let response: Response = client.get(&args.internal_file_url)
            .send().await
            .map_err(Box::from)?;

        if !response.status().is_success() {
            return Ok(())
        }
        let mut rp = RouteParams {
            canonical_route_name: format!("{}|{}", args.dongle_id, args.timestamp),
            device_dongle_id: args.dongle_id.clone(),
            url: "Not implemented".to_string(),
            ..Default::default()
        };

        // Check if the route has been added previously.
        let route = match routes::Model::find_route(&self.ctx.db,  &rp.canonical_route_name).await {
            Ok(route) => route,
            Err(_) => { 
                tracing::info!("Recieved file for a new route. Adding to DB: {}", &rp.canonical_route_name);
                match routes::Model::add_route(&self.ctx.db, &rp).await {
                    Ok(route) => route,
                    Err(e) => {
                        tracing::error!("Failed to add the default route: {}", &rp.canonical_route_name);
                        return Err(sidekiq::Error::Message(e.to_string()));
                    }
                }
            }
        };

        let canonical_name = format!("{}|{}--{}", args.dongle_id, args.timestamp, args.segment);
        let seg = match segments::Model::find_by_segment(&self.ctx.db, &canonical_name).await {
            Ok(segment) => segment, // The segment was added previously so here is the row.
            Err(e) => {  // Need to add the segment now.
                tracing::info!("Recieved file for a new route. Adding to DB: {}", &canonical_name);
                let default_segment_model = _entities::segments::Model { canonical_name: canonical_name, canonical_route_name: route.canonical_route_name, ..Default::default() };
                match default_segment_model.add_segment_self(&self.ctx.db).await {
                    Ok(segment) => segment, // The segment was added and here is the row.
                    Err(e) => return Err(sidekiq::Error::Message("Failed to add the default segment: ".to_string() + &e.to_string()))
                }
            }
        };
        let mut seg = seg.into_active_model();
        match args.file.as_str() {
            "rlog.bz2" =>  seg.rlog_url = ActiveValue::Set(format!("{}/connectdata/{}/{}/{}/{}", self.ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file)),
            "qlog.bz2" =>  {
                    match handle_qlog(&mut seg, response, &args, &self.ctx, &client).await {
                        Ok(_) => (),
                        Err(e) => return Err(sidekiq::Error::Message("Failed to handle qlog: ".to_string() + &e.to_string())),
                    }
                }
            "qcamera.ts" =>     seg.qcam_url = ActiveValue::Set(format!("{}/connectdata/{}/{}/{}/{}", self.ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file)),
            "fcamera.hvec" =>   seg.fcam_url = ActiveValue::Set(format!("{}/connectdata/{}/{}/{}/{}", self.ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file)),
            "dcamera.hvec" =>   seg.dcam_url = ActiveValue::Set(format!("{}/connectdata/{}/{}/{}/{}", self.ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file)),
            "ecamera.hvec" =>   seg.ecam_url = ActiveValue::Set(format!("{}/connectdata/{}/{}/{}/{}", self.ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file)),
            (f) => { tracing::info!("Got invalid file type: {}", f); return Ok(())} // TODO: Mark for immediate deletion and block this user
        }
        let seg_active_model = seg.into_active_model();
        match seg_active_model.update(&self.ctx.db).await {
            Ok(_) => {tracing::info!("Completed unlogging: {} in {:?}", args.internal_file_url, start.elapsed()); Ok(())},
            Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
        }
    }
}

async fn handle_qlog(
    seg: &mut segments::ActiveModel,
    response: Response, 
    args: &LogSegmentWorkerArgs, 
    ctx: &AppContext, 
    client: &Client
) -> worker::Result<()> {
    let bytes_stream = response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let stream_reader = StreamReader::new(bytes_stream);
    let mut bz2_decoder = BzDecoder::new(stream_reader);
    
    let mut decompressed_data = Vec::new();
    match bz2_decoder.read_to_end(&mut decompressed_data).await { 
        Ok(_)=> (), 
        Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
    };
    // Prepare route and segment parameters
    let writer = match parse_qlog(seg, decompressed_data, args, ctx).await { 
        Ok(writer) => writer, 
        Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
    };
    // Upload the processed data
    match upload_data(&client, &args.internal_file_url.replace(".bz2", ".unlog"), writer).await {
        Ok(()) => return Ok(()),
        Err(e) => return Err(sidekiq::Error::Message(e.to_string())),
    };
}

async fn parse_qlog(seg: &mut segments::ActiveModel, decompressed_data: Vec<u8>, args: &LogSegmentWorkerArgs, ctx: &AppContext) -> worker::Result<(Vec<u8>)> {
    seg.ulog_url = ActiveValue::Set(
        format!(
            "{}/useradmin/logs?url={}",
            ctx.config.server.full_url(),
            common::mkv_helpers::get_mkv_file_url(
                &format!("{}_{}--{}--{}",
                    args.dongle_id,
                    args.timestamp,
                    args.segment,
                    args.file.replace("bz2", "unlog")
                )
            ).await
        ));
    seg.qlog_url = ActiveValue::Set(format!("{}/connectdata/{}/{}/{}/{}", ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file));

    let mut writer = Vec::new();
    let mut cursor = Cursor::new(decompressed_data);
    let mut gps_seen = false;
    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
        let event = message_reader.get_root::<log_capnp::event::Reader>().map_err(Box::from)?;
        

        match event.which().map_err(Box::from)? {
            log_capnp::event::InitData(init_data) => {
                if let Ok(init_data) = init_data {}
                writeln!(writer, "{:#?}", event).map_err(Box::from)?;
            }
            log_capnp::event::GpsLocationExternal(gps) => {
                if let Ok(gps) = gps {
                    if !gps_seen { // gps is false the first time
                        gps_seen = true;
                        seg.hpgps = ActiveValue::Set(true);
                        seg.start_time_utc_millis = ActiveValue::Set(gps.get_unix_timestamp_millis());
                        seg.start_lat = ActiveValue::Set(gps.get_latitude());
                        seg.start_lng = ActiveValue::Set(gps.get_longitude());
                    }
                    seg.end_time_utc_millis = ActiveValue::Set(gps.get_unix_timestamp_millis());
                    seg.end_lat = ActiveValue::Set(gps.get_latitude());
                    seg.end_lng = ActiveValue::Set(gps.get_longitude());
                }
                writeln!(writer, "{:#?}", event).map_err(Box::from)?;
            }
            log_capnp::event::Thumbnail(thumbnail) => {}
            _ => {writeln!(writer, "{:#?}", event).map_err(Box::from)?;}
        }
    }
    Ok(writer)
}

async fn upload_data(client: &Client, url: &String, body: Vec<u8>) -> MyResult<()> {
    let response = client.put(url)
        .body(body)
        .send().await
        .map_err(Box::from)?;

    if !response.status().is_success() {
        tracing::info!("Response status: {}", response.status());
        return MyResult::Err(Error::Message("Failed to upload data".to_string()));
    }

    Ok(())
}