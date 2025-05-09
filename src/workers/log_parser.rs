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
use async_compression::tokio::{bufread, write::BzEncoder};
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

use crate::cereal::log_capnp;
use crate::common;
use crate::cereal::log_capnp::event as LogEvent;
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
        //let start_time = Instant::now();

        let mut keys = self.keys.lock().await;
        while keys.contains_key(&key) {
            drop(keys);
            //let wait_start = Instant::now();
            self.notify.notified().await;
            keys = self.keys.lock().await;
        }

        // Insert the key to indicate it is locked
        keys.insert(key, true);
        // TODO: Fix the global locking
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
        //pub static CLIENT: Lazy<Arc<Client>> = Lazy::new(|| Arc::new(Client::new()));
        pub static CLIENT: Lazy<Arc<Client>> = Lazy::new(|| {
            Arc::new(
                Client::builder()
                    .tcp_nodelay(true)
                    .connect_timeout(std::time::Duration::from_secs(1))
                    .timeout(std::time::Duration::from_secs(1)) // Response timeout
                    .pool_idle_timeout(Some(std::time::Duration::from_secs(10)))
                    .build()
                    .expect("Failed to create HTTP client")
            )
        });
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
        lock_manager.acquire_advisory_lock(&self.ctx.db, key).await
            .map_err(|e| sidekiq::Error::Message(format!("Failed to aquire advisory lock: {}", e)))?; // blocks here until lock aquired

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
        self.lock_manager.release_advisory_lock(&self.ctx.db, key).await
            .map_err(|e| sidekiq::Error::Message(format!("Failed to release advisory lock: {}", e)))?;


        let canonical_name = format!("{}|{}--{}", args.dongle_id, args.timestamp, args.segment);
        let key = super::log_helpers::calculate_advisory_lock_key(&canonical_name);
        self.lock_manager.acquire_advisory_lock(&self.ctx.db, key).await
            .map_err(|e| sidekiq::Error::Message(format!("Failed to aquire advisory lock: {}", e)))?; // blocks here until lock aquired
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
        //let mut ignore_uploads = None;
        let mut qlog_result: Option<QLogResult> = None;
        match args.file.as_str() {
            "rlog.bz2" | "rlog.zst" =>  {
                seg.rlog_url = ActiveValue::Set(format!("{api_endpoint}/connectdata/rlog/{}/{}/{}/{}",
                    args.dongle_id,
                    args.timestamp,
                    args.segment,
                    args.file));
                // match anonamize_rlog(&self.ctx, response, &client, &args).await {
                //     Ok(_) => (),
                //     Err(e) => return Err(sidekiq::Error::Message("Failed to anonamize rlog: ".to_string() + &e.to_string())),
                // }
                
            }
            "qlog.bz2" | "qlog.zst" =>  {
                    qlog_result = match handle_qlog(&mut seg, response, &args, &self.ctx, &client).await {
                        Ok(qlog_result) => Some(qlog_result),
                        Err(e) => {
                            tracing::error!("Failed to handle qlog: {}", &e.to_string());
                            return Err(sidekiq::Error::Message("Failed to handle qlog: ".to_string() + &e.to_string()))
                        },
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
            "fcamera.hevc" =>   seg.fcam_url = ActiveValue::Set(
                format!("{api_endpoint}/connectdata/fcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)
            ),
            "dcamera.hevc" =>   seg.dcam_url = ActiveValue::Set(
                format!("{api_endpoint}/connectdata/dcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)
            ),
            "ecamera.hevc" =>   seg.ecam_url = ActiveValue::Set(
                format!("{api_endpoint}/connectdata/ecam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)
            ),
            f => {
                tracing::error!("Got invalid file type: {}", f);
                //ignore_uploads = Some(true);
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
            active_route_model.git_remote = ActiveValue::Set(if log.git_remote.is_empty() {
                None
            } else {
                Some(log.git_remote)
            });
            active_route_model.git_commit = ActiveValue::Set(if log.git_commit.is_empty() {
                None
            } else {
                Some(log.git_commit)
            });
            active_route_model.git_branch = ActiveValue::Set(if log.git_branch.is_empty() {
                None
            } else {
                Some(log.git_branch)
            });
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
        self.lock_manager.release_advisory_lock(&self.ctx.db, key).await
            .map_err(|e| sidekiq::Error::Message(format!("Failed to release advisory lock: {}", e))
        )?;

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
    if let Some(mut last_seg) = segment_models.last(){
        last_seg = if last_seg.end_time_utc_millis == 0 && segment_models.len() > 1 {
            &segment_models[segment_models.len() - 2] // Sometimes the last seg is too short to have gps
        } else {
            last_seg
        };
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
    let mut calculated_start_time = 0;

    let mut gps_seen = false;
    for segment_model in segment_models {
        segment_numbers.push(segment_model.number);
    
        if segment_model.hpgps {
            miles += segment_model.miles;
            if !gps_seen {
                gps_seen = true;
                active_route_model.hpgps = ActiveValue::Set(true);
                calculated_start_time = segment_model.start_time_utc_millis - (segment_model.number as i64 * 60000);
                active_route_model.start_time_utc_millis = ActiveValue::Set(calculated_start_time);
                active_route_model.start_time =  ActiveValue::Set(DateTime::from_timestamp_millis(calculated_start_time));
                active_route_model.start_lat = ActiveValue::Set(segment_model.start_lat);
                active_route_model.start_lng = ActiveValue::Set(segment_model.start_lng);
            }
            
            active_route_model.end_time = ActiveValue::Set(DateTime::from_timestamp_millis(segment_model.end_time_utc_millis));
            active_route_model.end_time_utc_millis = ActiveValue::Set(segment_model.end_time_utc_millis);
            active_route_model.end_lat = ActiveValue::Set(segment_model.end_lat);
            active_route_model.end_lng = ActiveValue::Set(segment_model.end_lng);
        }

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

    // We have looped through all the segments. If none have gps, then we need 
    // to set the start time based on the first segment created_at which
    // is the time the first segment was uploaded. created_at is in naive UTC
    if !gps_seen {
        let first_seg = segment_models.first().unwrap();
        let created_time = first_seg.created_at.and_utc().timestamp_millis(); 
        active_route_model.start_time_utc_millis = ActiveValue::Set(created_time);
        active_route_model.start_time = ActiveValue::Set(Some(first_seg.created_at));
        calculated_start_time = created_time - (first_seg.number as i64 * 60000); // first segment number may not be 0
    }

    for segment_model in segment_models {
        let (seg_start_time, seg_end_time) = if segment_model.hpgps {
            (segment_model.start_time_utc_millis, segment_model.start_time_utc_millis)
        } else {
            let base_time = calculated_start_time + (segment_model.number as i64 * 60000);
            (base_time, base_time + 60000)
        };

        segment_start_times.push(seg_start_time);
        segment_end_times.push(seg_end_time);
    }

    active_route_model.length = ActiveValue::Set(miles);
    active_route_model.segment_start_times = ActiveValue::Set(segment_start_times.into());
    active_route_model.segment_end_times = ActiveValue::Set(segment_end_times.into());
    active_route_model.segment_numbers = ActiveValue::Set(segment_numbers.into());
    return Ok(());
}

async fn handle_qlog(
    seg: &mut segments::ActiveModel,
    response: Response,
    args: &LogSegmentWorkerArgs,
    ctx: &AppContext,
    client: &Client
) -> worker::Result<QLogResult> {
    let bytes_stream = response
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)
    );
    let stream_reader = StreamReader::new(bytes_stream);
    let mut decoder: std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>> = if args.file.ends_with(".bz2") {
        Box::pin(bufread::BzDecoder::new(stream_reader))
    } else if args.file.ends_with(".zst") {
        Box::pin(bufread::ZstdDecoder::new(stream_reader))
    } else {
        return Err(sidekiq::Error::Message("Invalid file type".to_string()));
    };

    let mut decompressed_data = Vec::new();

    match decoder.read_to_end(&mut decompressed_data).await {
        Ok(_)=> (),
        Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
    };

    Ok(parse_qlog(&client, seg, decompressed_data, args, ctx).await?)
}

struct QLogResult {
    car_fingerprint: String,
    git_branch: String,
    git_remote: String,
    git_commit: String,
    openpilot_version: String,
    device_type: log_capnp::init_data::DeviceType,
    start_time: i64,
    end_time: i64,
    total_time: i64,
}

impl Default for QLogResult {
    fn default() -> Self {
        Self {
            car_fingerprint: "mock".to_string(),
            git_branch: "".to_string(),
            git_remote: "".to_string(),
            git_commit: "".to_string(),
            openpilot_version: "".to_string(),
            device_type: log_capnp::init_data::DeviceType::Unknown,
            start_time: -1,
            end_time: -1,
            total_time: 0,
        }
    }
}

#[derive(Deserialize, Debug, Serialize)]
struct StateData {
    state: String,
    enabled: bool,
    alertStatus: i8,
}

#[derive(Deserialize, Debug, Serialize)]
struct JsonEvent {
    r#type: String,
    time: u64,
    route_offset_millis:u64,
    data: StateData,
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
                        args.file.replace("bz2", "unlog").replace(".zst", ".unlog")
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
    let mut events: Vec<serde_json::Value> = Vec::new();
    let mut qlog_result = QLogResult{..Default::default()};

    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
        let event = match message_reader.get_root::<LogEvent::Reader>() {
            Ok(event) => event,
            Err(e) => {tracing::warn!("Failed to get root: {:?}", e); continue}, // Skip parsing if we can't get the root
        };

        match event.which() {
            Err(_e) => {
                //tracing::trace!("Event type not in schema: {:?}", e);
                continue; // Skip this iteration if matching fails. This happends often
            }
            Ok(event_type) => {
                let log_mono_time = event.get_log_mono_time();
                match event_type {
                    LogEvent::GpsLocationExternal(gps) | LogEvent::GpsLocation(gps)=> {
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
        
                            } else if qlog_result.start_time == -1{
                                qlog_result.start_time = gps_ts;
                                seg.start_time_utc_millis = ActiveValue::Set(qlog_result.start_time);
                            }
                            if qlog_result.end_time < gps_ts {
                                qlog_result.end_time = gps_ts;
                                seg.end_time_utc_millis = ActiveValue::Set(gps_ts);
                            }
                        }
                        writeln!(unlog_data, "{:#?}", event).ok();
                    }
                    LogEvent::DeviceState(device_state) => {
                        if let Ok(device_state) = device_state {
        
                            if device_state.get_started() {
                                onroad_mono_time = Some(device_state.get_started_mono_time());
                            }
                        }
                        writeln!(unlog_data, "{:#?}", event).ok();
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
                        qlog_result.car_fingerprint = car_params
                            .ok()
                            .and_then(|params| params.get_car_fingerprint().ok())
                            .map_or_else(String::new, |fp| fp.to_string().unwrap_or_default());
                        writeln!(unlog_data, "{:#?}", event).ok();
                    }
                    LogEvent::InitData(init_data) => {
                        if let Ok(init_data) = init_data {
                            qlog_result.git_branch = init_data
                                .get_git_branch().ok()
                                .map_or_else(String::new, |d| d.to_string().unwrap_or_default());
                            qlog_result.git_commit = init_data
                                .get_git_commit().ok()
                                .map_or_else(String::new, |d| d.to_string().unwrap_or_default());
                            qlog_result.git_remote = init_data
                                .get_git_remote().ok()
                                .map_or_else(String::new, |d| d.to_string().unwrap_or_default());
                            qlog_result.openpilot_version = init_data
                                .get_version().ok()
                                .map_or_else(String::new, |d| d.to_string().unwrap_or_default());
                            qlog_result.device_type = init_data
                                .get_device_type().ok()
                                .map_or(log_capnp::init_data::DeviceType::Unknown, |d| d);
                        }
        
                        writeln!(unlog_data, "{:#?}", event).ok();
                    }
                    LogEvent::OnroadEvents(onroad_event) => {
                        if let Ok(onroad_event) = onroad_event {
                            if onroad_event.len() == 0 {
                                continue;
                            }
                            for car_event in onroad_event.iter() {
                                let mut state = "disabled";
                                let mut enabled: bool = false;
                                let mut alert_status = 0;
                                let mut _name = car_event.get_name().ok();
                                let _no_entry = car_event.get_no_entry();
                                let warning = car_event.get_warning();
                                let _user_disable = car_event.get_user_disable();
                                let soft_disable = car_event.get_soft_disable();
                                let immediate_disable = car_event.get_immediate_disable();
                                let _pre_enable = car_event.get_pre_enable();
                                let _permanent = car_event.get_permanent();
                                let overridden = car_event.get_override_lateral() || car_event.get_override_longitudinal();
                                if car_event.get_enable() || car_event.get_pre_enable() {
                                    state = "enabled";
                                    enabled = true;
                                }
                                if overridden {
                                    state = "overriding";
                                    enabled = true
                                }
                                if car_event.get_user_disable() || car_event.get_soft_disable() || car_event.get_immediate_disable() {
                                    state = "disabled";
                                }
                                if immediate_disable {
                                    alert_status = 2;
                                }
                                if soft_disable || warning {
                                    alert_status = 1;
                                }

                                if let Some(onroad_mono_time) = onroad_mono_time{
                                    events.push(
                                        serde_json::json!({
                                            "type": "state",
                                            "time": log_mono_time,
                                            "route_offset_millis": (log_mono_time - onroad_mono_time) / 1000000,
                                            "data": {
                                                "state": state,
                                                "enabled": enabled,
                                                "alertStatus": alert_status,
                                            }
                                        })
                                    )
                                }
                            }
                        }
                        writeln!(unlog_data, "{:#?}", event).ok();
                    }
                    LogEvent::PandaStates(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::Can(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::Sendcan(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::ErrorLogMessage(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::LogMessage(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::LiveParameters(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::LiveTorqueParameters(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::ManagerState(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::NavInstruction(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::UploaderState(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    LogEvent::QcomGnss(_) => {writeln!(unlog_data, "{:#?}", event).ok();},
                    _ => continue, //writeln!(writer, "{:#?}", event).map_err(Box::from)?, // unlog everything?
                }
            }
        }
    }

    if let (Some(last_lat), Some(last_lng)) = (last_lat, last_lng) {
        seg.end_lat = ActiveValue::Set(last_lat);
        seg.end_lng = ActiveValue::Set(last_lng);
        seg.miles = ActiveValue::Set((total_meters_traveled*0.000621371) as f32);
    }

    let coords_url = common::mkv_helpers::get_mkv_file_url(
        &format!("{}_{}--{}--coords.json", args.dongle_id, args.timestamp, args.segment)
    );

    let events_url = common::mkv_helpers::get_mkv_file_url(
        &format!("{}_{}--{}--events.json", args.dongle_id, args.timestamp, args.segment)
    );
    
    upload_data(client, &coords_url, serde_json::to_vec(&coordinates).unwrap_or_default()).await;
    upload_data(client, &events_url, serde_json::to_vec(&events).unwrap_or_default()).await;
    upload_data(&client, 
        &args.internal_file_url
            .replace(".bz2", ".unlog")
            .replace(".zst", ".unlog"),
        unlog_data
    ).await;

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
            encoder.encode_image(&DynamicImage::ImageRgba8(final_img)).ok();
            img_bytes
        };

        let sprite_url = common::mkv_helpers::get_mkv_file_url(
            &format!("{}_{}--{}--sprite.jpg", args.dongle_id, args.timestamp, args.segment)
        );
        tracing::trace!("Image proc took: {:?}", img_proc_start.elapsed());
        upload_data(client, &sprite_url, img_bytes).await;
    }

    qlog_result.total_time = coordinates
        .last()
        .and_then(|last_coordinate| last_coordinate.get("t"))
        .and_then(|t| t.as_i64())  // Convert to i64 if it's a number
        .unwrap_or(0);  // Default to 0 in case of any error

    Ok(qlog_result)
}

async fn upload_data(client: &Client, url: &str, body: Vec<u8>) {
    match client.put(url).body(body).send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                tracing::trace!("Uploaded data to {} with status {}", url, status);
            } else {
                tracing::error!("Failed to upload data to {} with status {}", url, status);
            }
        }
        Err(e) => {
            tracing::error!("Request to {} failed: {}", url, e);
        }
    }
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