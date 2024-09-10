use std::{
    env,
    time::Instant,
    sync::Arc,
    collections::HashMap,
    io::{Write, Cursor}
};
use tokio_util::io::StreamReader;
use tokio::{
    io::{AsyncWriteExt, AsyncReadExt},
    sync::{Mutex, Notify},
};
use reqwest::{Client, Response};
use rayon::prelude::*;
use ffmpeg_next::{format as ffmpeg_format, Error as FfmpegError};
use tempfile::NamedTempFile;
use async_compression::tokio::{bufread::BzDecoder, write::BzEncoder};
use futures_util::StreamExt;
use futures::stream::TryStreamExt; // for stream::TryStreamExt to use try_next
use once_cell::sync::Lazy;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use capnp::{
    message::ReaderOptions,
    serialize::{read_message, write_message}
};

use log_capnp::event as LogEvent;
use crate::common;
use crate::cereal::log_capnp;
use crate::models::_entities::{devices, routes, segments};

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

use sea_orm::{DatabaseConnection, DbErr};
use async_trait::async_trait;

pub struct LockManager {
    keys: Mutex<HashMap<u32, bool>>,
    notify: Notify,
}

impl LockManager {
    fn new() -> Self {
        LockManager {
            keys: Mutex::new(HashMap::new()),
            notify: Notify::new(),
        }
    }

    pub async fn acquire_advisory_lock(&self, _db: &DatabaseConnection, key: u32) -> Result<(), DbErr> {
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
        /// TODO: Fix the global locking
        // This is the global lock (literally for servers around the globe accessing the same db)
        // tracing::trace!("Attempting to acquire advisory lock with key: {}", key);
        // let sql = format!("SELECT pg_advisory_lock({})", key);
        // db.execute(Statement::from_string(db.get_database_backend(), sql)).await?;
        // tracing::trace!("Successfully acquired advisory lock with key: {}", key);


        Ok(())
    }

    pub async fn release_advisory_lock(&self, _db: &DatabaseConnection, key: u32) -> Result<(), DbErr> {
        let mut keys = self.keys.lock().await;
        if keys.remove(&key).is_some() {
            // Notify all waiting threads that a key has been removed
            self.notify.notify_waiters();
        }
        // tracing::trace!("Releasing advisory lock with key: {}", key);
        // let sql = format!("SELECT pg_advisory_unlock({})", key);
        // db.execute(Statement::from_string(db.get_database_backend(), sql)).await?;
        // tracing::trace!("Successfully released advisory lock with key: {}", key);

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
        let api_endpoint: String = env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set");

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
                    url: format!("{api_endpoint}/connectdata/{}/{}_{}",
                        args.dongle_id,
                        args.dongle_id,
                        args.timestamp),
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
        let segment = match segments::Model::find_one(&self.ctx.db, &canonical_name).await {
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
                        match segments::Model::find_one(&self.ctx.db, &canonical_name).await {
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
        let mut qlog_result: Option<QLogResult> = None;
        match args.file.as_str() {
            "rlog.bz2" =>  {
                seg.rlog_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/rlog/{}/{}/{}/{}",
                    args.dongle_id,
                    args.timestamp,
                    args.segment,
                    args.file));
                match anonamize_rlog(&self.ctx, response, &client, &args).await {
                    Ok(_) => (),
                    Err(e) => return Err(sidekiq::Error::Message("Failed to anonamize rlog: ".to_string() + &e.to_string())),
                }
            }
            "qlog.bz2" =>  {
                    qlog_result = match handle_qlog(&mut seg, response, &args, &self.ctx, &client).await {
                        Ok(qlog_result) => Some(qlog_result),
                        Err(e) => return Err(sidekiq::Error::Message("Failed to handle qlog: ".to_string() + &e.to_string())),
                    };
                }
            "qcamera.ts" => {
                match get_qcam_duration(response).await {
                    Ok(duration) => seg.qcam_duration = ActiveValue::Set(duration),
                    Err(_e) => tracing::error!("failed to get duration"),
                }
                seg.qcam_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/qcam/{}/{}/{}/{}",
                    args.dongle_id,
                    args.timestamp,
                    args.segment,
                    args.file));
            }
            "fcamera.hevc" =>   seg.fcam_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/fcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "dcamera.hevc" =>   seg.dcam_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/dcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "ecamera.hevc" =>   seg.ecam_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/ecam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
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
            Ok(segments) => {
                //segments.retain(|segment| segment.qlog_url != ""); // exclude ones wher the qlog is missing
                segments
            }
            Err(e) => {
                tracing::error!("Failed to get segment models for route: {}. DB Error {}", &route_model.fullname, e.to_string());
                return Err(sidekiq::Error::Message(e.to_string()));
            }
        };
        let mut active_route_model = route_model.into_active_model();
        if let Some(log) = qlog_result {
            active_route_model.platform = ActiveValue::Set(log.car_fingerprint);
        }
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
    }
    // all segments in route
    let mut segment_start_times = vec![];
    let mut segment_end_times = vec![];
    let mut segment_numbers = vec![];
    let mut miles = 0.0;
    let mut hpgps = false;

    for segment_model in segment_models {
        miles += segment_model.miles;
        segment_start_times.push(segment_model.start_time_utc_millis);
        segment_end_times.push(segment_model.end_time_utc_millis);
        segment_numbers.push(segment_model.number);

        if segment_model.hpgps { // if this segment has accurate timestamp
            let calculated_start_time = (segment_model.start_time_utc_millis - (segment_model.number as i64 * 60000));
            active_route_model.start_time_utc_millis = ActiveValue::Set(calculated_start_time);
            active_route_model.start_time =  ActiveValue::Set(DateTime::from_timestamp_millis(calculated_start_time));
        }

        hpgps |= segment_model.hpgps;
        if segment_model.qlog_url != "" {
            active_route_model.maxqlog = ActiveValue::Set(segment_model.number as i32);
        }
        if segment_model.rlog_url != "" {
            active_route_model.maxlog = ActiveValue::Set(segment_model.number as i32);
        }
        if segment_model.qcam_url != "" {
            active_route_model.maxqcamera = ActiveValue::Set(segment_model.number as i32);
        }
        if segment_model.fcam_url != "" {
            active_route_model.maxcamera = ActiveValue::Set(segment_model.number as i32);
        }
        if segment_model.dcam_url != "" {
            active_route_model.maxdcamera = ActiveValue::Set(segment_model.number as i32);
        }
        if segment_model.ecam_url != "" {
            active_route_model.maxecamera = ActiveValue::Set(segment_model.number as i32);
        }
    }

    active_route_model.length = ActiveValue::Set(miles);
    active_route_model.segment_start_times = ActiveValue::Set(segment_start_times.into());
    active_route_model.segment_end_times = ActiveValue::Set(segment_end_times.into());
    active_route_model.segment_numbers = ActiveValue::Set(segment_numbers.into());
    active_route_model.hpgps = ActiveValue::Set(hpgps);
    return Ok(());
}

async fn handle_qlog(
    seg: &mut segments::ActiveModel,
    response: Response,
    args: &LogSegmentWorkerArgs,
    ctx: &AppContext,
    client: &Client
) -> worker::Result<QLogResult> {
    let bytes_stream = response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let stream_reader = StreamReader::new(bytes_stream);
    let mut bz2_decoder = BzDecoder::new(stream_reader);

    let mut decompressed_data = Vec::new();

    match bz2_decoder.read_to_end(&mut decompressed_data).await {
        Ok(_)=> (),
        Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
    };

    Ok(parse_qlog(&client, seg, decompressed_data, args, ctx).await?)
}

struct QLogResult {
    car_fingerprint: String,
    start_time: i64,
    end_time: i64,
    total_time: i64,
}

async fn parse_qlog(
    client: &Client,
    seg: &mut segments::ActiveModel,
    decompressed_data: Vec<u8>,
    args: &LogSegmentWorkerArgs,
    _ctx: &AppContext
) -> worker::Result<QLogResult> {
    let api_endpoint = env::var("API_ENDPOINT").expect("API_ENDPOINT env variable not set");
    seg.ulog_url = ActiveValue::Set(
        format!(
            "{api_endpoint}/connectdata/logs?url={}",
                common::mkv_helpers::get_mkv_file_url(
                    &format!("{}_{}--{}--{}",
                        args.dongle_id,
                        args.timestamp,
                        args.segment,
                        args.file.replace("bz2", "unlog")
                    )
            )
        ));
    seg.qlog_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/qlog/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file));

    let mut unlog_data = Vec::new();
    let mut cursor = Cursor::new(decompressed_data);
    let mut onroad_mono_time: Option<u64> = None;
    let mut gps_seen = false;
    let mut thumbnails: Vec<Vec<u8>> = Vec::new();
    let mut total_meters_traveled = 0.0; // gets converted to miles
    let mut last_lat = None;
    let mut last_lng = None;
    let mut coordinates: Vec<serde_json::Value> = Vec::new();
    let mut start_time: i64= -1;
    let mut end_time: i64 = -1;
    let mut car_fingerprint: String = "mock".to_string();

    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
        let event = message_reader.get_root::<LogEvent::Reader>().map_err(Box::from)?;

        match event.which().map_err(Box::from)? {
            LogEvent::GpsLocationExternal(gps) | LogEvent::GpsLocation(gps)=> {
                let log_mono_time = event.get_log_mono_time();
                if let Ok(gps) = gps {
                    let gps_ts = gps.get_unix_timestamp_millis();
                    if (gps.get_flags() % 2 == 1) || gps.get_has_fix() { // has fix
                        let lat = gps.get_latitude();
                        let lng = gps.get_longitude();
                        let speed = gps.get_speed();
                        if !gps_seen { // gps_seen is false the first time
                            gps_seen = true;
                            seg.hpgps = ActiveValue::Set(true);
                            seg.start_time_utc_millis = ActiveValue::Set(gps_ts);
                            seg.start_lat = ActiveValue::Set(lat);
                            seg.start_lng = ActiveValue::Set(lng);
                        }

                        // Calculate distance if we have previous coordinates
                        if let (Some(last_lat), Some(last_lng)) = (last_lat, last_lng) {
                            let meters = super::log_helpers::haversine_distance(last_lat, last_lng, lat, lng);
                            total_meters_traveled += meters;

                            if let Some(onroad_mono_time) = onroad_mono_time{
                                let route_time = (log_mono_time - onroad_mono_time) / 1000000000; // time since the start of route
                                coordinates.push(serde_json::json!({
                                    "t": route_time,
                                    "lat": lat,
                                    "lng": lng,
                                    "speed": speed,
                                    "dist": meters,
                                }));
                            }
                        }

                        // Update last coordinates
                        last_lat = Some(lat);
                        last_lng = Some(lng);

                    } else if start_time == -1{
                        start_time = gps_ts;
                        seg.start_time_utc_millis = ActiveValue::Set(start_time);
                    }
                    if end_time < gps_ts {
                        end_time = gps_ts;
                        seg.end_time_utc_millis = ActiveValue::Set(gps_ts);
                    }
                }
                writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?;
            }
            LogEvent::DeviceState(device_state) => {
                if let Ok(device_state) = device_state {

                    if device_state.get_started() {
                        onroad_mono_time = Some(device_state.get_started_mono_time());
                    }
                }
                writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?
            }

            LogEvent::Thumbnail(thumbnail) => {
                // take the jpg and add it to the array of the other jpgs.
                // after we get all the jpgs, put them together into a 1x12 jpg and downscale to 1280x96
                if let Ok(thumbnail) = thumbnail {
                    // Assuming the thumbnail data is a JPEG image
                    let image_data = thumbnail.get_thumbnail().map_err(Box::from)?; // len is 9682
                    //let img = image::load_from_memory(image_data).map_err(Box::from)?; // len is 436692
                    thumbnails.push(image_data.to_vec());
                }
            }
            LogEvent::CarParams(car_params) => {
                car_fingerprint = car_params
                    .ok()
                    .and_then(|params| params.get_car_fingerprint().ok())
                    .map_or_else(String::new, |fp| fp.to_string().unwrap_or_default());
                writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?
            }
            LogEvent::InitData(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::PandaStates(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::Can(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::Sendcan(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::ErrorLogMessage(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::LogMessage(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::LiveParameters(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::LiveTorqueParameters(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::ManagerState(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::NavInstruction(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::OnroadEvents(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::UploaderState(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            LogEvent::QcomGnss(_) => writeln!(unlog_data, "{:#?}", event).map_err(Box::from)?,
            _ => continue, //writeln!(writer, "{:#?}", event).map_err(Box::from)?, // unlog everything?
        }
    }

    if let (Some(last_lat), Some(last_lng)) = (last_lat, last_lng) {
        seg.end_lat = ActiveValue::Set(last_lat);
        seg.end_lng = ActiveValue::Set(last_lng);
        seg.miles = ActiveValue::Set((total_meters_traveled*0.000621371) as f32);
    }

    let json_url = common::mkv_helpers::get_mkv_file_url(
        &format!("{}_{}--{}--coords.json", args.dongle_id, args.timestamp, args.segment)
    );
    upload_data(client, &json_url, serde_json::to_vec(&coordinates).map_err(Box::from)?).await?;
    upload_data(&client, &args.internal_file_url.replace(".bz2", ".unlog"), unlog_data).await?;
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

    let total_time = coordinates
        .last()
        .and_then(|last_coordinate| last_coordinate.get("t"))
        .and_then(|t| t.as_i64())  // Convert to i64 if it's a number
        .unwrap_or(0);  // Default to 0 in case of any error

    Ok(QLogResult{ car_fingerprint, start_time, end_time, total_time})
}

async fn upload_data(client: &Client, url: &str, body: Vec<u8>) -> worker::Result<()> {
    let response = client.put(url)
        .body(body)
        .send().await
        .map_err(Box::from)?;

    if !response.status().is_success() {
        tracing::debug!("Response status: {}", response.status());
        return Err(sidekiq::Error::Message(format!("Failed to upload data to {}", url)));
    }
    tracing::debug!("Uploaded {url} Response status: {}", response.status());
    Ok(())
}

async fn get_qcam_duration(response: Response) -> Result<f32, FfmpegError> {
    // Create a temporary file to store the video data
    let temp_file: NamedTempFile = NamedTempFile::new().unwrap();
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
    ffmpeg_next::init()?;
    let context = ffmpeg_format::input(&temp_file.path())?;
    let duration = context.duration() as f32 / 1_000_000.0;
    Ok(duration)
}

const MAX_FOLDER_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
const MAX_FILE_COUNT: usize = 10;

macro_rules! reader_to_builder {
    ($set_fn:ident, $event_variant:ident, $event:expr, $event_builder:expr) => {
        if let Ok(reader) = $event {
            let _ = $event_builder.$set_fn(reader);
        }
    };
}

async fn anonamize_rlog(_ctx: &AppContext, response: Response, _client: &Client, args: &LogSegmentWorkerArgs) -> Result<(), Error> {
    let start_time = Instant::now();

    let bytes_stream = response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
    let stream_reader = StreamReader::new(bytes_stream);
    let mut bz2_decoder = BzDecoder::new(stream_reader);

    let mut decompressed_data = Vec::new();
    match bz2_decoder.read_to_end(&mut decompressed_data).await {
        Ok(_) => (),
        Err(e) => return Err(Error::Message(e.to_string())),
    };
    let decompress_duration = start_time.elapsed();
    tracing::trace!("Decompressing file stream took: {:?}", decompress_duration);

    let process_start = Instant::now();
    let mut cursor = Cursor::new(decompressed_data);
    let mut writer = Vec::new();
    let mut car_fingerprint: Option<String> = None;
    let mut total_meters_traveled = 0.0; // gets converted to miles
    let mut last_lat = None;
    let mut last_lng = None;
    let mut gps_seen = false;
    while let Ok(message_reader) = read_message(&mut cursor, capnp::message::ReaderOptions::default()) {
        let event = match message_reader.get_root::<LogEvent::Reader>() {
            Ok(event) => event,
            Err(_) => continue,
        };
        let mut message_builder = ::capnp::message::Builder::new_default();
        let mut event_builder = message_builder.init_root::<LogEvent::Builder>();
        event_builder.set_log_mono_time(event.get_log_mono_time());
        let event_type = event.which().unwrap();
        match event_type {
            LogEvent::GpsLocationExternal(gps) | LogEvent::GpsLocation(gps)=> {
                if let Ok(gps) = gps {
                    if (gps.get_flags() % 2) == 1 { // has fix
                        let lat = gps.get_latitude();
                        let lng = gps.get_longitude();
                        if !gps_seen { // gps_seen is false the first time
                            gps_seen = true;
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
                    }
                }
                continue;
            },
            LogEvent::CarState(reader) => reader_to_builder!(set_car_state, event_type, reader, event_builder),
            LogEvent::LiveParameters(reader) => reader_to_builder!(set_live_parameters, event_type, reader, event_builder),
            LogEvent::CarControl(reader) => reader_to_builder!(set_car_control, event_type, reader, event_builder),
            LogEvent::LateralPlanDEPRECATED(reader) => reader_to_builder!(set_lateral_plan_d_e_p_r_e_c_a_t_e_d, event_type, reader, event_builder),
            LogEvent::CarOutput(reader) => reader_to_builder!(set_car_output, event_type, reader, event_builder),
            LogEvent::ModelV2(reader) => reader_to_builder!(set_model_v2, event_type, reader, event_builder),
            LogEvent::LiveTorqueParameters(reader) => reader_to_builder!(set_live_torque_parameters, event_type, reader, event_builder),
            LogEvent::LiveCalibration(reader) => reader_to_builder!(set_live_calibration, event_type, reader, event_builder),
            LogEvent::Sendcan(reader) => reader_to_builder!(set_sendcan, event_type, reader, event_builder),
            LogEvent::Can(reader) => reader_to_builder!(set_can, event_type, reader, event_builder),
            LogEvent::LongitudinalPlan(reader) => reader_to_builder!(set_longitudinal_plan, event_type, reader, event_builder),
            LogEvent::CarParams(reader) => {
                reader_to_builder!(set_car_params, event_type, reader, event_builder);
                if let Ok(mut reader) = reader {
                    car_fingerprint = Some(reader.get_car_fingerprint().unwrap().to_string().unwrap());
                }
            }
            LogEvent::LiveLocationKalman(llk) => {
                if let Ok(mut llk_reader) = llk {
                    let mut builder = capnp::message::Builder::new_default();
                    let mut llk_builder: log_capnp::live_location_kalman::Builder = builder.get_root::<log_capnp::live_location_kalman::Builder>().unwrap();
                    llk_builder.set_angular_velocity_calibrated(llk_reader.get_angular_velocity_calibrated().unwrap());
                    llk_builder.set_orientation_n_e_d(llk_reader.get_orientation_n_e_d().unwrap());
                    llk_builder.set_calibrated_orientation_n_e_d(llk_reader.get_calibrated_orientation_n_e_d().unwrap());
                    llk_reader = llk_builder.into_reader();
                    event_builder.set_live_location_kalman(llk_reader).unwrap();
                }
            },
            _ => continue,
        }
        write_message(&mut writer, &message_builder).unwrap();
    }

    let process_duration = process_start.elapsed();
    tracing::trace!("Processing messages took: {:?}", process_duration);
    if car_fingerprint.is_none() || (total_meters_traveled <= 40.0) {
        return Ok(());
    }
    let platform = car_fingerprint.unwrap_or("mock".to_string()).clone();

    let compress_start = Instant::now();
    let compress_duration = compress_start.elapsed();
    tracing::trace!("Compressing and writing file took: {:?}", compress_duration);
    let prefix = "/tmp/anonlogs/";
    let local_path = format!("{prefix}/{}/{}", &platform, &args.dongle_id);
    let persistent_dir = std::path::Path::new(&local_path);
    if !persistent_dir.exists() {
        match std::fs::create_dir_all(persistent_dir) {
            Ok(_) => (),
            Err(e) => tracing::error!("Failed to create dir: {e}"),
        }
    }
    let temp_path = persistent_dir.join(format!("{}--{}--{}", &args.timestamp, &args.segment, &args.file));
    let temp_file = tokio::fs::File::create(&temp_path).await?;
    let mut async_writer = tokio::io::BufWriter::new(temp_file);
    let mut async_bz_encoder = BzEncoder::with_quality(&mut async_writer, async_compression::Level::Default);

    async_bz_encoder.write_all(&writer).await?;
    async_bz_encoder.shutdown().await?;
    async_writer.flush().await?;

    let folder_size = get_folder_size(persistent_dir)?;
    let file_count = get_file_count(persistent_dir)?;

    if folder_size >= MAX_FOLDER_SIZE || file_count >= MAX_FILE_COUNT {
        let repo_path = format!("{}/{}", &platform, &args.dongle_id);
        let _ = upload_folder_to_huggingface(&prefix, &repo_path).await; // Upload everything
        clear_directory(persistent_dir)?; // only delete the dongle_id path to avoid a race
    }

    let total_duration = start_time.elapsed();
    tracing::trace!("Total operation took: {:?}", total_duration);

    Ok(())
}

fn get_folder_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut size = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            size += metadata.len();
        }
    }
    Ok(size)
}

fn get_file_count(path: &std::path::Path) -> std::io::Result<usize> {
    let mut count = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.metadata()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

async fn upload_folder_to_huggingface(folder_path: &str, repo_path: &str) -> Result<(), std::io::Error> {
    let repo_id = "MoreTorque/rlogs";

    // Clone the local_path and repo_path into owned Strings
    let folder_path2 = folder_path.to_string();

    // Spawn a blocking task to run the Python script
    let result = tokio::task::spawn_blocking(move || {
        let status = std::process::Command::new("huggingface-cli")
            .arg("upload")
            .arg(repo_id)
            .arg(&folder_path2)
            .arg("/")
            .arg("--repo-type=dataset")
            .status();

        status
    })
    .await
    .expect("Failed to run blocking task");

    match result {
        Ok(status) => {
            if status.success() {
                tracing::info!("Folder {} uploaded to Hugging Face successfully", folder_path);
            } else {
                tracing::error!("Failed to upload folder {} to Hugging Face", folder_path);
            }
        }
        Err(e) => {
            tracing::error!("Failed to execute process: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

fn clear_directory(path: &std::path::Path) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                clear_directory(&entry_path)?;
                std::fs::remove_dir(&entry_path)?;
            } else {
                std::fs::remove_file(&entry_path)?;
            }
        }
    }
    Ok(())
}