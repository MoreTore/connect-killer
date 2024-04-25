use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use capnp::message::ReaderOptions;
use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter, Cursor};
use std::path::Path;
use anyhow::{Result, Context};
//use bzip2::read::BzDecoder;
use crate::cereal::log_capnp::{self, init_data};
use reqwest::{Body ,Client};
use crate::common;

use std::time::Instant;
use crate::models::{ segments::SegmentParams, _entities::segments};
                
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

        let segment_base_url = format!("{}/connectdata/{}/{}/{}", 
                                       self.ctx.config.server.full_url(), 
                                       args.dongle_id,
                                       args.timestamp,
                                       args.segment);
        let mut sp = SegmentParams {
            canonical_name: format!("{}|{}--{}", args.dongle_id, args.timestamp, args.segment),
            url: "test".to_string(),
            qlog_url: format!("{}/{}", segment_base_url, args.file),
            qcam_url: None,
            rlog_url: None,
            fcam_url: None,
            dcam_url: None,
            ecam_url: None,
            start_time_utc_millis: 0,
            end_time_utc_millis: 0,
            end_lng: Some(0.0),
            start_lng: Some(0.0),
            end_lat: Some(0.0),
            start_lat: Some(0.0),
            hgps: false,
            proc_log: false,
            proc_camera: false,
            can: false,
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
                            if !sp.hgps { // gps is false the first time
                                sp.hgps = true;
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
        let decoded_file_url = args.internal_file_url.replace("rlog.bz2", "rlog.unlog");
        upload_data(
            &client, &decoded_file_url, writer).await.map_err(Box::from)?;
        segments::Model::add_segment(&self.ctx.db, &sp).await.map_err(Box::from)?;

        tracing::info!("Completed unlogging: {} in {:?}", args.internal_file_url, start.elapsed());
        Ok(())
    }
}

fn process_data(bytes: &[u8]) -> worker::Result<Vec<u8>> {
    let mut cursor = Cursor::new(bytes);
    let mut writer = Vec::new();
    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
        let event = message_reader.get_root::<log_capnp::event::Reader>().map_err(Box::from)?;
        writeln!(writer, "{:#?}", event).map_err(Box::from)?;
        match event.which().map_err(Box::from)? {
            log_capnp::event::InitData(init_data) => {
                //let init_data = init_data.map_err(Box::from)?;
            }
            _ => {}
        }
    }

    Ok(writer)
}

async fn upload_data(client: &Client, url: &str, body: Vec<u8>) -> MyResult<()> {
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