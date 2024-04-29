use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use capnp::message::ReaderOptions;
use migration::m20240424_000004_segments::Segments as SegmentFields;
use crate::models::segments::{Model, ActiveModel};
use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter, Cursor};
use std::path::Path;
use anyhow::{Result, Context};
//use bzip2::read::BzDecoder;
use crate::cereal::log_capnp::{self, init_data};
use reqwest::{Body ,Client};
use crate::common;

use std::time::Instant;
use crate::models::{ segments::SegmentParams, _entities::segments, _entities::routes, routes::RouteParams};
                
use futures::stream::TryStreamExt; // for stream::TryStreamExt to use try_next
use tokio_util::io::StreamReader;
use async_compression::tokio::bufread::BzDecoder;
use tokio::io::AsyncReadExt; // for read_to_end
pub struct QlogParserWorker {
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
pub struct QlogParserWorkerArgs {
    pub internal_file_url: String,
    pub dongle_id        : String,
    pub timestamp        : String,
    pub segment          : String,
    pub file             : String,
    pub create_time      : u64, // This is the time the call was made to the worker.
}

impl worker::AppWorker<QlogParserWorkerArgs> for QlogParserWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

#[async_trait]
impl worker::Worker<QlogParserWorkerArgs> for QlogParserWorker {
    async fn perform(&self, args: QlogParserWorkerArgs) -> worker::Result<()> {
        let start = Instant::now();
        tracing::trace!("Starting QlogParser for URL: {}", args.internal_file_url);
        let client = Client::new();
        //self.ctx.db
        
        // Send the request and handle the response
        let response = client.get(&args.internal_file_url)
            .send().await
            .map_err(Box::from)?;

        if !response.status().is_success() {
            return Ok(())
        }

        let bytes_stream = response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
        let stream_reader = StreamReader::new(bytes_stream);
        let mut bz2_decoder = BzDecoder::new(stream_reader);

        let mut decompressed_data = Vec::new();
        bz2_decoder.read_to_end(&mut decompressed_data).await.map_err(Box::from)?;
        let ulog_url = format!(
            "{}/useradmin/logs?url={}", 
            self.ctx.config.server.full_url(), 
            common::mkv_helpers::get_mkv_file_url(
                &format!("{}_{}--{}--{}",
                args.dongle_id, 
                args.timestamp,
                args.segment,
                args.file.replace("bz2","unlog") )).await);
        let mut sp = SegmentParams {
            canonical_name: format!("{}|{}--{}", args.dongle_id, args.timestamp, args.segment),
            ulog_url: ulog_url.into(),
            qlog_url: format!("{}/connectdata/{}/{}/{}/{}",  self.ctx.config.server.full_url(), args.dongle_id, args.timestamp, args.segment, args.file),
            proc_log: 4,
            ..Default::default()
        };

        let mut rp = RouteParams {
            canonical_route_name: format!("{}|{}", args.dongle_id, args.timestamp),
            device_dongle_id: args.dongle_id,
            url: "Not implemented".to_string(),
            ..Default::default()
        };
        
        let mut writer = Vec::new();
        {
            let mut cursor = Cursor::new(decompressed_data);
            while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
                let event = message_reader.get_root::<log_capnp::event::Reader>().map_err(Box::from)?;
                writeln!(writer, "{:#?}", event).map_err(Box::from)?;
                match event.which().map_err(Box::from)? {
                    log_capnp::event::InitData(init_data) => {
                        if let Ok(init_data) = init_data {
                        }
                    }
                    log_capnp::event::GpsLocationExternal(gps) => {
                        if let Ok(gps) = gps {
                            if !sp.hpgps { // gps is false the first time
                                sp.hpgps = true;
                                sp.start_time_utc_millis = gps.get_unix_timestamp_millis();
                                sp.start_lat = Some(gps.get_latitude());
                                sp.start_lng = Some(gps.get_longitude());
                            }
                            sp.end_time_utc_millis = gps.get_unix_timestamp_millis();
                            sp.end_lat = Some(gps.get_latitude());
                            sp.end_lng = Some(gps.get_longitude());
                        }
                    }
                    _ => {}
                }
            }
        }
        // Upload the processed data
        let decoded_file_url = args.internal_file_url.replace(".bz2", ".unlog");
        upload_data(
            &client, &decoded_file_url, writer).await.map_err(Box::from)?;
        let res = routes::Model::find_route(&self.ctx.db, &rp.canonical_route_name).await;
        match res {
            Ok(_) => (), // route already added
            Err(_) => {
                routes::Model::add_route(&self.ctx.db, &rp).await;
                ();
            }
        };
        segments::Model::add_segment(&self.ctx.db, &sp).await.map_err(Box::from)?;

        tracing::info!("Completed unlogging: {} in {:?}", args.internal_file_url, start.elapsed());
        Ok(())
    }
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