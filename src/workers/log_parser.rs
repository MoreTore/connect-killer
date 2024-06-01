// Apology for the Messy Code
/* 
I apologize for the messiness and potential lack of clarity in this code. 
It evolved organically over time with various additions and modifications to handle multiple aspects 
of the log processing. This includes handling database interactions, HTTP requests, image processing, 
and video duration extraction, among other tasks. As a result, the code may appear disjointed and less organized than ideal.

I recognize that there are areas that could benefit from refactoring and clearer documentation to improve 
readability and maintainability. Specifically, there are opportunities to modularize the code further, improve 
error handling consistency, and enhance the overall structure.

Your understanding and patience are greatly appreciated. I am committed to improving this codebase and 
welcome any suggestions you may have for making it cleaner and more efficient.

Sincerely,
Ryleymcc
*/



use image::{codecs::jpeg::JpegEncoder, DynamicImage, GenericImageView, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use capnp::message::ReaderOptions;
use crate::models::_entities::{devices, routes, segments};
//                     devices,
//                     users,
//                     authorized_users};
use std::io::{Write, Cursor};
//use bzip2::read::BzDecoder;
use crate::cereal::log_capnp::{self};
use reqwest::{Client, Response};
use crate::common;

use std::time::Instant;
//use crate::models::{ segments::SegmentParams, _entities::segments, _entities::routes, routes::RouteParams};
                
use futures::stream::TryStreamExt; // for stream::TryStreamExt to use try_next
use tokio_util::io::StreamReader;
use async_compression::tokio::bufread::BzDecoder;
use tokio::io::AsyncReadExt; // for read_to_end
use rayon::prelude::*;
use ffmpeg::{format as ffmpeg_format, Error as FfmpegError};
use tempfile::NamedTempFile;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use std::collections::HashMap;
use tokio::sync::{Mutex, Notify};

use std::sync::Arc;
use once_cell::sync::Lazy;

pub struct LogSegmentWorker {
    pub ctx: AppContext,
    pub lock_manager: Arc<LockManager>,
    pub client: Arc<Client>,
}
#[derive(Deserialize, Debug, Serialize)]
pub struct LogSegmentWorkerArgs {
    pub internal_file_url: String,
    pub dongle_id        : String,
    pub timestamp        : String,
    pub segment          : String,
    pub file             : String,
    pub create_time      : i64, // This is the time the call was made to the worker.
}

use sea_orm::{DatabaseConnection, DbErr, Statement};
use async_trait::async_trait;

pub struct LockManager {
    keys: Mutex<HashMap<i64, bool>>,
    notify: Notify,
}

impl LockManager {
    fn new() -> Self {
        LockManager {
            keys: Mutex::new(HashMap::new()),
            notify: Notify::new(),
        }
    }

    pub async fn acquire_advisory_lock(&self, db: &DatabaseConnection, key: i64) -> Result<(), DbErr> {
        // This is the local server lock
        let mut keys = self.keys.lock().await;
        while keys.contains_key(&key) {
            // Drop the lock before awaiting and re-acquire it after being notified
            drop(keys);
            self.notify.notified().await;
            keys = self.keys.lock().await;
        }

        // Insert the key to indicate it is locked
        keys.insert(key, true);
        // This is the global lock (literally for servers around the globe accessing the same db)
        tracing::trace!("Attempting to acquire advisory lock with key: {}", key);
        let sql = format!("SELECT pg_advisory_lock({})", key);
        db.execute(Statement::from_string(db.get_database_backend(), sql)).await?;
        tracing::trace!("Successfully acquired advisory lock with key: {}", key);

        Ok(())
    }

    pub async fn release_advisory_lock(&self, db: &DatabaseConnection, key: i64) -> Result<(), DbErr> {
        let mut keys = self.keys.lock().await;
        if keys.remove(&key).is_some() {
            // Notify all waiting threads that a key has been removed
            self.notify.notify_waiters();
        }
        tracing::trace!("Releasing advisory lock with key: {}", key);
        let sql = format!("SELECT pg_advisory_unlock({})", key);
        db.execute(Statement::from_string(db.get_database_backend(), sql)).await?;
        tracing::trace!("Successfully released advisory lock with key: {}", key);

        Ok(())
    }
}


impl worker::AppWorker<LogSegmentWorkerArgs> for LogSegmentWorker {
    fn build(ctx: &AppContext) -> Self {
        static LOCK_MANAGER: Lazy<Arc<LockManager>> = Lazy::new(|| Arc::new(LockManager::new()));
        pub static CLIENT: Lazy<Arc<Client>> = Lazy::new(|| Arc::new(Client::new()));
        Self { ctx: ctx.clone(), lock_manager: Arc::clone(&LOCK_MANAGER) , client: Arc::clone(&CLIENT)}
    }
}

#[async_trait]
impl worker::Worker<LogSegmentWorkerArgs> for LogSegmentWorker {
    async fn perform(&self, args: LogSegmentWorkerArgs) -> worker::Result<()> {
        let lock_manager = self.lock_manager.clone();
        let start_time = Instant::now();
        tracing::trace!("Starting QlogParser for URL: {}", args.internal_file_url);
        let client = self.client.clone();

        // check if the device is in the database
        let _device_model = match devices::Model::find_device(&self.ctx.db, &args.dongle_id).await {
            Ok(device) => device,
            Err(e) => {
                tracing::info!("Recieved file from an unregistered device. {} or DB Error: {}", &args.dongle_id, e.to_string());
                return Ok(())
            }
        };

        let canonical_route_name = format!("{}|{}", args.dongle_id, args.timestamp);
        let key = super::log_helpers::calculate_advisory_lock_key(&canonical_route_name);
        lock_manager.acquire_advisory_lock(&self.ctx.db, key).await.map_err(|e| sidekiq::Error::Message(format!("Failed to aquire advisory lock: {}", e)))?; // blocks here until lok aquired
        
        let route_model = match routes::Model::find_route(&self.ctx.db,  &canonical_route_name).await {
            Ok(route) => route,
            Err(e) => { 
                tracing::trace!("Recieved file for a new route. Adding to DB: {} or Db Error: {}", &canonical_route_name, e);
                let default_route_model = routes::Model {
                    fullname: format!("{}|{}", args.dongle_id, args.timestamp),
                    device_dongle_id: args.dongle_id.clone(),
                    url: format!("https://connect-api.duckdns.org/connectdata/{}/{}_{}", args.dongle_id, args.dongle_id, args.timestamp),
                    ..Default::default()
                };
                match default_route_model.add_route_self(&self.ctx.db).await {
                    Ok(route) => route,
                    Err(e) => {
                        tracing::error!("Failed to add the default route: {} with Error: {}", &canonical_route_name, e.to_string());
                        match routes::Model::find_route(&self.ctx.db, &canonical_route_name).await {
                            Ok(route) => {
                                tracing::error!("But it was added in a separate thread!");
                                route
                            }
                            Err(_) => return Err(sidekiq::Error::Message("Failed to add the default route: ".to_string() + &e.to_string())),
                        }
                    }
                }
            }
        };
        self.lock_manager.release_advisory_lock(&self.ctx.db, key).await.map_err(|e| sidekiq::Error::Message(format!("Failed to release advisory lock: {}", e)))?;

        
        let canonical_name = format!("{}|{}--{}", args.dongle_id, args.timestamp, args.segment);
        let key = super::log_helpers::calculate_advisory_lock_key(&canonical_name);
        self.lock_manager.acquire_advisory_lock(&self.ctx.db, key).await.map_err(|e| sidekiq::Error::Message(format!("Failed to aquire advisory lock: {}", e)))?; // blocks here until lok aquired
        let segment = match segments::Model::find_by_segment(&self.ctx.db, &canonical_name).await {
            Ok(segment) => segment, // The segment was added previously so here is the row.
            Err(e) => {  // Need to add the segment now.
                tracing::trace!("Received file for a new segment. Adding to DB: {} or DB Error: {}", &canonical_name, e);
                let default_segment_model = segments::Model {
                    canonical_name: canonical_name.clone(),
                    canonical_route_name: canonical_route_name.clone(),
                    number: args.segment.parse::<i16>().unwrap_or(0),
                    ..Default::default()
                };
                match default_segment_model.add_segment_self(&self.ctx.db).await {
                    Ok(segment) => segment, // The segment was added and here is the row.
                    Err(e) => {
                        tracing::trace!("Failed to add the default segment {}: {}", &canonical_name, e);
                        match segments::Model::find_by_segment(&self.ctx.db, &canonical_name).await {
                            Ok(segment) => {
                                tracing::trace!("But it was added in a separate thread!");
                                segment
                            }
                            Err(_) => return Err(sidekiq::Error::Message("Failed to add the default segment: ".to_string() + &e.to_string())),
                        }
                    }
                }
            }
        };

        // Make sure we have the data in the key value store. Maybe not needed later
        let response = match client.get(&args.internal_file_url).send().await {
            Ok(response) => {
                let status = response.status();
                tracing::trace!("Got Ok response with status {status}");
                if !status.is_success() {
                    return Ok(());
                }
                response
            }
            Err(e) => {
                tracing::error!("GET request failed: {}", format!("{}", e));
                return Err(sidekiq::Error::Message(e.to_string()));
            }
        };
        

        let mut seg = segment.into_active_model();
        let mut ignore_uploads = None;
        match args.file.as_str() {
            "rlog.bz2" =>  seg.rlog_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/rlog/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "qlog.bz2" =>  {
                    match handle_qlog(&mut seg, response, &args, &self.ctx, &client).await {
                        Ok(_) => (),
                        Err(e) => return Err(sidekiq::Error::Message("Failed to handle qlog: ".to_string() + &e.to_string())),
                    }
                }
            "qcamera.ts" => {
                match get_qcam_duration(response).await {
                    Ok(duration) => seg.qcam_duration = ActiveValue::Set(duration),
                    Err(_e) => tracing::error!("failed to get duration"),
                }
                seg.qcam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/qcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file));
            }
            "fcamera.hevc" =>   seg.fcam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/fcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "dcamera.hevc" =>   seg.dcam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/dcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "ecamera.hevc" =>   seg.ecam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/ecam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            f => { 
                tracing::error!("Got invalid file type: {}", f);
                ignore_uploads = Some(true);
                return Ok(())
            } // TODO: Mark for immediate deletion and block this user
        }
        //let seg_active_model = seg.into_active_model();
        match seg.update(&self.ctx.db).await {
            Ok(_) => (),
            Err(e) => {
                tracing::error!("Failed to update segment: {}. DB Error: {}", &args.internal_file_url, e.to_string());
                return Err(sidekiq::Error::Message(e.to_string()));
            }
        }
        let segment_models = match segments::Model::find_segments_by_route(&self.ctx.db, &route_model.fullname).await {
            Ok(mut segments) => {
                segments.retain(|segment| segment.start_time_utc_millis != 0); // exclude ones wher the qlog is missing
                segments.sort_by(|a,b| a.start_time_utc_millis.cmp(&b.start_time_utc_millis));
                segments
            }
            Err(e) => {
                tracing::error!("Failed to get segment models for route: {}. DB Error {}", &route_model.fullname, e.to_string());
                return Err(sidekiq::Error::Message(e.to_string()));
            }
        };
        let mut active_route_model = route_model.into_active_model();
        //let mut active_device_model = device_model.into_active_model();
        update_route_info(&self.ctx, &mut active_route_model, &segment_models).await?;
        //update_device_info(&self.ctx, &mut active_device_model, &active_route_model, &ignore_uploads).await?;

        match active_route_model.update(&self.ctx.db).await {
            Ok(_) => (),
            Err(e) => {
                tracing::error!("Failed to update active route model. DB Error {}", e.to_string());
                return Err(sidekiq::Error::Message(e.to_string()));
            }
        }
        self.lock_manager.release_advisory_lock(&self.ctx.db, key).await.map_err(|e| sidekiq::Error::Message(format!("Failed to release advisory lock: {}", e)))?;

        //active_device_model.update(&self.ctx.db).await.map_err(|e| sidekiq::Error::Message(e.to_string()))?;
        tracing::info!("Completed unlogging: {} in {:?}", args.internal_file_url, start_time.elapsed());
        return Ok(())
    }
}

// async fn update_device_info(
//     ctx: &AppContext,
//     active_device_model: &mut devices::ActiveModel,
//     active_route_model: &routes::ActiveModel,
//     ignore_uploads: &Option<bool>,
// ) -> worker::Result<()> {
    
//     return Ok(());
//}

async fn update_route_info(
    _ctx: &AppContext,
    active_route_model: &mut routes::ActiveModel,
    segment_models: &Vec<segments::Model>,
) -> worker::Result<()> {
    // First segment in route
    if let Some(first_seg) = segment_models.first() {
        active_route_model.start_time = ActiveValue::Set(DateTime::from_timestamp_millis(first_seg.start_time_utc_millis));
        active_route_model.start_time_utc_millis = ActiveValue::Set(first_seg.start_time_utc_millis);
        active_route_model.start_lat = ActiveValue::Set(first_seg.start_lat);
        active_route_model.start_lng = ActiveValue::Set(first_seg.start_lng);
    } else {
        tracing::error!("segment_models is empty!");
        return Err(sidekiq::Error::Message("segment_models is empty!".to_string()))
    }
    // last segment in route
    if let Some(last_seg) = segment_models.last() {
        active_route_model.end_time = ActiveValue::Set(DateTime::from_timestamp_millis(last_seg.end_time_utc_millis));
        active_route_model.end_time_utc_millis = ActiveValue::Set(last_seg.end_time_utc_millis);
        active_route_model.end_lat = ActiveValue::Set(last_seg.end_lat);
        active_route_model.end_lng = ActiveValue::Set(last_seg.end_lng);
    } else {
        tracing::error!("segment_models is empty!");
        return Err(sidekiq::Error::Message("segment_models is empty!".to_string()))
    }
    // all segments in route
    let mut segment_start_times = vec![];
    let mut segment_end_times = vec![];
    let mut segment_numbers = vec![];
    let mut maxcamera = 0;
    let mut maxdcamera = 0;
    let mut maxecamera = 0;
    let mut maxlog = 0;
    let mut maxqlog = 0;
    let _proclog = 0;
    let _procqcamera = 0;
    let _procqlog = 0;
    let mut maxqcamera = 0;
    let mut miles = 0.0;

    for segment_model in segment_models {
        miles += segment_model.miles;
        segment_start_times.push(segment_model.start_time_utc_millis);
        segment_end_times.push(segment_model.end_time_utc_millis);
        segment_numbers.push(segment_model.number);
        if segment_model.qlog_url != "" {
            maxqlog+= 1;
            active_route_model.maxqlog = ActiveValue::Set(maxqlog);
        }
        if segment_model.rlog_url != "" {
            maxlog+= 1;
            active_route_model.maxlog = ActiveValue::Set(maxlog);
        }
        if segment_model.qcam_url != "" {
            maxqcamera+= 1;
            active_route_model.maxqcamera = ActiveValue::Set(maxqcamera);
        }
        if segment_model.fcam_url != "" {
            maxcamera+= 1;
            active_route_model.maxcamera = ActiveValue::Set(maxcamera);
        }
        if segment_model.dcam_url != "" {
            maxdcamera+= 1;
            active_route_model.maxdcamera = ActiveValue::Set(maxdcamera);
        }
        if segment_model.ecam_url != "" {
            maxecamera+= 1;
            active_route_model.maxecamera = ActiveValue::Set(maxecamera);
        }
    }
    active_route_model.length = ActiveValue::Set(miles);
    active_route_model.segment_start_times = ActiveValue::Set(segment_start_times.into());
    active_route_model.segment_end_times = ActiveValue::Set(segment_end_times.into());
    active_route_model.segment_numbers = ActiveValue::Set(segment_numbers.into());
    return Ok(());
    // return Ok(active_route_model);
    // // Update the route in the db
    // match active_route_model.update(&ctx.db).await {
    //     Ok(_) => return Ok(()),
    //     Err(e) => return Err(sidekiq::Error::Message(e.to_string())),
    // }
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
    let writer = match parse_qlog(&client, seg, decompressed_data, args, ctx).await { 
        Ok(writer) => writer, 
        Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
    };
    // Upload the processed data
    match upload_data(&client, &args.internal_file_url.replace(".bz2", ".unlog"), writer).await {
        Ok(()) => return Ok(()),
        Err(e) => return Err(sidekiq::Error::Message(e.to_string())),
    };
}

async fn parse_qlog(
    client: &Client, 
    seg: &mut segments::ActiveModel, 
    decompressed_data: Vec<u8>, 
    args: &LogSegmentWorkerArgs, 
    _ctx: &AppContext
) -> worker::Result<Vec<u8>> {
    seg.ulog_url = ActiveValue::Set(
        format!(
            "https://connect-api.duckdns.org/connectdata/logs?url={}",
            common::mkv_helpers::get_mkv_file_url(
                &format!("{}_{}--{}--{}",
                    args.dongle_id,
                    args.timestamp,
                    args.segment,
                    args.file.replace("bz2", "unlog")
                )
            )
        ));
    seg.qlog_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/qlog/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file));

    let mut writer = Vec::new();
    let mut cursor = Cursor::new(decompressed_data);
    let mut gps_seen = false;
    let mut thumbnails: Vec<Vec<u8>> = Vec::new();
    let mut total_meters_traveled = 0.0; // gets converted to miles
    let mut last_lat = None;
    let mut last_lng = None;

    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
        let event = message_reader.get_root::<log_capnp::event::Reader>().map_err(Box::from)?;

        match event.which().map_err(Box::from)? {
            log_capnp::event::GpsLocationExternal(gps) | log_capnp::event::GpsLocation(gps)=> {
                if let Ok(gps) = gps {
                    if (gps.get_flags() % 2) == 1 { // has fix
                        let lat = gps.get_latitude();
                        let lng = gps.get_longitude();
                        if !gps_seen { // gps_seen is false the first time
                            gps_seen = true;
                            seg.hpgps = ActiveValue::Set(true);
                            seg.start_time_utc_millis = ActiveValue::Set(gps.get_unix_timestamp_millis());
                            seg.start_lat = ActiveValue::Set(lat);
                            seg.start_lng = ActiveValue::Set(lng);
                        }
        
                        // Calculate distance if we have previous coordinates
                        if let (Some(last_lat), Some(last_lng)) = (last_lat, last_lng) {
                            total_meters_traveled += super::log_helpers::haversine_distance(
                                last_lat, last_lng, lat, lng
                            );
                        }
        
                        // Update last coordinates
                        last_lat = Some(lat);
                        last_lng = Some(lng);
                        seg.end_time_utc_millis = ActiveValue::Set(gps.get_unix_timestamp_millis());
                    }
                }
                writeln!(writer, "{:#?}", event).map_err(Box::from)?;
            }
            log_capnp::event::Thumbnail(thumbnail) => {
                // take the jpg and add it to the array of the other jpgs.
                // after we get all the jpgs, put them together into a 1x12 jpg and downscale to 1280x96
                if let Ok(thumbnail) = thumbnail {
                    // Assuming the thumbnail data is a JPEG image
                    let image_data = thumbnail.get_thumbnail().map_err(Box::from)?; // len is 9682
                    //let img = image::load_from_memory(image_data).map_err(Box::from)?; // len is 436692
                    thumbnails.push(image_data.to_vec());
                }
            }
            log_capnp::event::InitData(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::PandaStates(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::DeviceState(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::Can(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::Sendcan(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::ErrorLogMessage(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::GpsLocationExternal(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::LiveParameters(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::LiveTorqueParameters(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            log_capnp::event::CarParams(_) => writeln!(writer, "{:#?}", event).map_err(Box::from)?,
            _ => continue,
        }
    }
    if let (Some(last_lat), Some(last_lng)) = (last_lat, last_lng) {
        seg.end_lat = ActiveValue::Set(last_lat);
        seg.end_lng = ActiveValue::Set(last_lng);
        seg.miles = ActiveValue::Set((total_meters_traveled*0.000621371) as f32);
    }

    let img_proc_start = Instant::now();
    if !thumbnails.is_empty() {
        // Downscale each thumbnail in parallel
        let downscaled_thumbnails: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = thumbnails.par_iter()
            .map(|image_data| {
                let img = image::load_from_memory(image_data).expect("Failed to load image");
                img.resize_exact(1536 / 12, 80, image::imageops::FilterType::Lanczos3).to_rgba8()
            })
            .collect();

        // Combine the downscaled images sequentially
        let combined_width = (1536 / 12) * downscaled_thumbnails.len() as u32;
        let mut combined_img = ImageBuffer::new(combined_width, 80);

        for (i, thumbnail) in downscaled_thumbnails.iter().enumerate() {
                    let offset = i as u32 * (1536 / 12);
                    for y in 0..80 {
                        for x in 0..(1536 / 12) {
                            let pixel = thumbnail.get_pixel(x, y);
                            combined_img.put_pixel(x + offset, y, *pixel);
                        }
                    }
        }

        // Create the final image with a height of 80px
        let mut final_img = ImageBuffer::new(combined_width, 80);
        image::imageops::overlay(&mut final_img, &DynamicImage::ImageRgba8(combined_img), 0, 0);

        // Convert the final image to a byte vector
        let img_bytes = {
            let mut img_bytes: Vec<u8> = Vec::new();
            let mut encoder = JpegEncoder::new_with_quality(&mut img_bytes, 80);
            encoder.encode_image(&DynamicImage::ImageRgba8(final_img)).map_err(Box::from)?;
            img_bytes
        };

        let sprite_url = common::mkv_helpers::get_mkv_file_url(
            &format!("{}_{}--{}--sprite.jpg", args.dongle_id, args.timestamp, args.segment)
        );
        tracing::trace!("Image proc took: {:?}", img_proc_start.elapsed());
        upload_data(client, &sprite_url, img_bytes).await?;
    }
    Ok(writer)
}




async fn upload_data(client: &Client, url: &String, body: Vec<u8>) -> worker::Result<()> {
    let response = client.put(url)
        .body(body)
        .send().await
        .map_err(Box::from)?;

    if !response.status().is_success() {
        tracing::debug!("Response status: {}", response.status());
        return Err(sidekiq::Error::Message(format!("Failed to upload data to {}", url)));
    }

    Ok(())
}

async fn get_qcam_duration(response: Response) -> Result<f32, FfmpegError> {
    // Create a temporary file to store the video data
    let temp_file = NamedTempFile::new().unwrap();
    let mut temp_file_async = tokio::fs::File::from_std(temp_file.reopen().unwrap());
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => temp_file_async.write_all(&chunk).await.unwrap(),
            Err(e) => {
                tracing::error!("streaming error: {}", e.to_string());
            }
        }
    }

    // Close the file to ensure all data is written
    temp_file_async.sync_all().await.unwrap();

    // Use FFmpeg to open the file and get the duration
    ffmpeg::init()?;
    let context = ffmpeg_format::input(&temp_file.path())?;
    let duration = context.duration() as f32 / 1_000_000.0;
    Ok(duration)
}
