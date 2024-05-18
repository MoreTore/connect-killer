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


pub struct LogSegmentWorker {
    pub ctx: AppContext,
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

        // check if the device is in the database
        let _device = match devices::Model::find_device(&self.ctx.db, &args.dongle_id).await {
            Ok(device) => device,
            Err(e) => {
                tracing::info!("Recieved file from an unregistered device. {}", &args.dongle_id);
                return Ok(())
            }
        };
        
        let canonical_route_name = format!("{}|{}", args.dongle_id, args.timestamp);
        let route = match routes::Model::find_route(&self.ctx.db,  &canonical_route_name).await {
            Ok(route) => route,
            Err(_) => { 
                tracing::info!("Recieved file for a new route. Adding to DB: {}", &canonical_route_name);
                let default_route_model = routes::Model {
                    fullname: format!("{}|{}", args.dongle_id, args.timestamp),
                    device_dongle_id: args.dongle_id.clone(),
                    url: format!("https://connect-api.duckdns.org/connectdata/{}/{}_{}", args.dongle_id, args.dongle_id, args.timestamp),
                    ..Default::default()
                };
                match default_route_model.add_route_self(&self.ctx.db).await {
                    Ok(route) => route,
                    Err(e) => {
                        tracing::error!("Failed to add the default route: {}", &canonical_route_name);
                        return Err(sidekiq::Error::Message(e.to_string()));
                    }
                }
            }
        };

        let canonical_name = format!("{}|{}--{}", args.dongle_id, args.timestamp, args.segment);
        let segment = match segments::Model::find_by_segment(&self.ctx.db, &canonical_name).await {
            Ok(segment) => segment, // The segment was added previously so here is the row.
            Err(_) => {  // Need to add the segment now.
                tracing::info!("Recieved file for a new segment. Adding to DB: {}", &canonical_name);
                let default_segment_model = segments::Model { canonical_name: canonical_name.clone(), 
                                                                                canonical_route_name: route.fullname.clone(), 
                                                                                number: args.segment.parse::<i16>().unwrap_or(0), 
                                                                                ..Default::default() };
                match default_segment_model.add_segment_self(&self.ctx.db).await {
                    Ok(segment) => segment, // The segment was added and here is the row.
                    Err(e) => {
                        tracing::error!("Failed to add the default segment {}: {}", &canonical_name, e);
                        return Err(sidekiq::Error::Message("Failed to add the default segment: ".to_string() + &e.to_string()))
                    }
                }
            }
        };
        let mut seg = segment.into_active_model();
        match args.file.as_str() {
            "rlog.bz2" =>  seg.rlog_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/rlog/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "qlog.bz2" =>  {
                    match handle_qlog(&mut seg, response, &args, &self.ctx, &client).await {
                        Ok(_) => (),
                        Err(e) => return Err(sidekiq::Error::Message("Failed to handle qlog: ".to_string() + &e.to_string())),
                    }
                }
            "qcamera.ts" =>     seg.qcam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/qcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "fcamera.hvec" =>   seg.fcam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/fcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "dcamera.hvec" =>   seg.dcam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/dcam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            "ecamera.hvec" =>   seg.ecam_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/ecam/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file)),
            f => { tracing::info!("Got invalid file type: {}", f); return Ok(())} // TODO: Mark for immediate deletion and block this user
        }
        //let seg_active_model = seg.into_active_model();
        match seg.update(&self.ctx.db).await {
            Ok(_) => {tracing::info!("Completed unlogging: {} in {:?}", args.internal_file_url, start.elapsed());},
            Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
        }
        update_route_info(&self.ctx, route).await
    }
}

async fn update_route_info(
    ctx: &AppContext,
    route_model: routes::Model
) -> worker::Result<()> {
    // Get segments
    let mut segment_models: Vec<segments::Model> = match segments::Model::find_segments_by_route(&ctx.db, &route_model.fullname).await {
        Ok(segments) => segments,
        Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
    };
    // sort by start time
    segment_models.sort_by(|a,b| a.start_time_utc_millis.cmp(&b.start_time_utc_millis));
    // convert to active model for updating
    let mut active_route_model = route_model.into_active_model();
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
    let mut proclog = 0;
    let mut procqcamera = 0;
    let mut procqlog = 0;
    let mut maxqcamera = 0;

    for segment_model in &segment_models {
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
    active_route_model.segment_start_times = ActiveValue::Set(segment_start_times.into());
    active_route_model.segment_end_times = ActiveValue::Set(segment_end_times.into());
    active_route_model.segment_numbers = ActiveValue::Set(segment_numbers.into());

    // Update the route in the db
    match active_route_model.update(&ctx.db).await {
        Ok(_) => return Ok(()),
        Err(e) => return Err(sidekiq::Error::Message(e.to_string())),
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

async fn parse_qlog(client: &Client, seg: &mut segments::ActiveModel, decompressed_data: Vec<u8>, args: &LogSegmentWorkerArgs, ctx: &AppContext) -> worker::Result<Vec<u8>> {
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
            ).await
        ));
    seg.qlog_url = ActiveValue::Set(format!("https://connect-api.duckdns.org/connectdata/qlog/{}/{}/{}/{}", args.dongle_id, args.timestamp, args.segment, args.file));

    let mut writer = Vec::new();
    let mut cursor = Cursor::new(decompressed_data);
    let mut gps_seen = false;
    let mut thumbnails: Vec<Vec<u8>> = Vec::new();

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
            _ => {writeln!(writer, "{:#?}", event).map_err(Box::from)?;}
        }
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

        // Create the final image with a height of 96px
        let mut final_img = ImageBuffer::new(1536, 96);
        image::imageops::overlay(&mut final_img, &DynamicImage::ImageRgba8(combined_img), 0, 0);

        // Fill the bottom 16px with black in parallel
        final_img.par_chunks_mut(1536 * 4).skip(80).for_each(|row| {
            for chunk in row.chunks_mut(4) {
                chunk.copy_from_slice(&[0, 0, 0, 255]);
            }
        });

        // Convert the final image to a byte vector
        let img_bytes = {
            let mut img_bytes: Vec<u8> = Vec::new();
            let mut encoder = JpegEncoder::new_with_quality(&mut img_bytes, 80);
            encoder.encode_image(&DynamicImage::ImageRgba8(final_img)).map_err(Box::from)?;
            img_bytes
        };

        let sprite_url = common::mkv_helpers::get_mkv_file_url(
            &format!("{}_{}--{}--sprite.jpg", args.dongle_id, args.timestamp, args.segment)
        ).await;
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
        tracing::info!("Response status: {}", response.status());
        return Err(sidekiq::Error::Message("Failed to upload data".to_string()));
    }

    Ok(())
}