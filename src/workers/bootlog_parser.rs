use capnp::message::ReaderOptions;
use futures::TryStreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use loco_rs::prelude::*;
use tokio::io::AsyncReadExt;
use std::time::Instant;
use std::io::Write;
use crate::{cereal::log_capnp, common, models::_entities::{self, bootlogs}};

pub struct BootlogParserWorker {
    pub ctx: AppContext,
}

struct ParsedLog {
    data: Vec<u8>,
    date_time: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct BootlogParserWorkerArgs {
    pub internal_file_url: String,
    pub dongle_id: String,
    pub file_name: String,
    pub create_time: i64, // time we got it
}

impl worker::AppWorker<BootlogParserWorkerArgs> for BootlogParserWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
}

#[async_trait]
impl worker::Worker<BootlogParserWorkerArgs> for BootlogParserWorker {
    async fn perform(&self, args: BootlogParserWorkerArgs) -> worker::Result<()> {
        println!("=================BootlogParser=======================");
        let start = Instant::now();
        tracing::trace!("Starting BootlogParser for URL: {}", args.internal_file_url);
        let client = Client::new();
        // Make sure we have the data in the key value store
        let response = client.get(&args.internal_file_url)
            .send().await
            .map_err(Box::from)?;
    
        if !response.status().is_success() {
            return Ok(())
        }
        // check if the device is in the database
        let _device = match _entities::devices::Model::find_device(&self.ctx.db, &args.dongle_id).await {
            Some(device) => device,
            None => {
                tracing::info!("Recieved file from an unregistered device. Do something: {}", &args.dongle_id);
                return Ok(())
            }
        };
        let bytes_stream = response.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
        let stream_reader = tokio_util::io::StreamReader::new(bytes_stream);
        let mut bz2_decoder = async_compression::tokio::bufread::BzDecoder::new(stream_reader);
        
        let mut decompressed_data = Vec::new();
        match bz2_decoder.read_to_end(&mut decompressed_data).await { 
            Ok(_)=> (), 
            Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
        };
        let parsed_log = match parse_bootlog(decompressed_data).await {
            Ok(parsed_log) => parsed_log,
            Err(e) => return Err(sidekiq::Error::Message(e.to_string()))
        };

        // add bootlog to db
        match _entities::bootlogs::Model::add_bootlog(
            &self.ctx.db,
            &args.dongle_id,
            &format!("{}/connectdata/bootlog/{}",
                self.ctx.config.server.full_url(), 
                args.internal_file_url.split("/").last()
                    .expect("Failed to access the last segment of the internal file URL")),
            &format!("{}/useradmin/logs?url={}",
                self.ctx.config.server.full_url(),
                common::mkv_helpers::get_mkv_file_url(
                    &format!("{}_{}",
                        &args.dongle_id,
                        &args.file_name.replace(".bz2", ".unlog")
                    )
                ).await
            ),
            &parsed_log.date_time).await 
        {
            Ok(_) => {
                tracing::info!("bootlog added to db");
            }
            Err(e) => {
                tracing::error!("Failed to add bootlog to db {}", e);
            }
        }

        match upload_data(&client, &args.internal_file_url.replace(".bz2", ".unlog"), parsed_log.data).await {
            Ok(()) => return Ok(()),
            Err(e) => return Err(sidekiq::Error::Message(e.to_string())),
        };

    }
}


async fn parse_bootlog(decompressed_data: Vec<u8>,) -> worker::Result<ParsedLog> {
    let mut writer: Vec<u8> = Vec::new();
    let mut cursor: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(decompressed_data);
    let mut date_string: String = "".to_string();
    while let Ok(message_reader) = capnp::serialize::read_message(&mut cursor, ReaderOptions::default()) {
        let event: log_capnp::event::Reader = message_reader.get_root::<log_capnp::event::Reader>().map_err(Box::from)?;
        //writeln!(writer, "{:#?}", event).map_err(Box::from)?;
        match event.which().map_err(Box::from)? {
            log_capnp::event::Boot(log) => {
                let bootlog = log.unwrap();
                if bootlog.has_launch_log() {
                    writeln!(writer, "{:#?}", bootlog.get_launch_log().unwrap()).map_err(Box::from)?;
                }
                if bootlog.has_pstore() {
                    writeln!(writer, "{:#?}", bootlog.get_pstore().unwrap()).map_err(Box::from)?;
                }
                if bootlog.has_commands() {
                    let data = bootlog.get_commands().unwrap();
                    let re_date = regex::Regex::new(r"end at \w+ (\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \w+.").unwrap();

                    if let Some(caps) = re_date.captures(&format!("{:?}", data)) {
                        if let Some(matched) = caps.get(1) {
                            println!("Extracted string: {}", matched.as_str());
                            // 2023-11-21 15:10:55
                            // 2023-11-21--15-10-55
                            date_string = matched.as_str().to_string().replace(":", "-").replace(" ", "--");
                            println!("Date found: {}", date_string);
                        } else {
                            println!("No 'end at' timestamp found.");
                        }
                    } else {
                        println!("No matches found.");
                    }
                    writeln!(writer, "{:?}", data).map_err(Box::from)?;
                }
            }
            _ => ()
        }
    }
    Ok(ParsedLog { data: writer, date_time: date_string})
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