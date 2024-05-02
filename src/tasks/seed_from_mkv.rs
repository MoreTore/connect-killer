use std::collections::BTreeMap;
use regex::Regex;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::{Value, from_str};
use crate::{common, workers::qlog_parser::{LogSegmentWorker, LogSegmentWorkerArgs}};
use loco_rs::prelude::*;

pub struct SeedFromMkv;

#[async_trait]
impl Task for SeedFromMkv {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "seed_from_mkv".to_string(),
            detail: "Task generator".to_string(),
        }
    }

    async fn run(&self, app_context: &AppContext, _vars: &BTreeMap<String, String>) -> Result<()> {
        println!("Task SeedFromMkv generated");

        let client = Client::new();

        // Get all keys from the MKV server
        let query = common::mkv_helpers::list_keys_starting_with("").await;
        let response = client.get(&query).send().await.unwrap();

        if !response.status().is_success() {
            tracing::info!("Failed to get keys");
            return Ok(());
        }

        let body = response.text().await.unwrap();
        let json: Value = from_str(&body).unwrap(); // Convert response text into JSON

        // Extract keys from the JSON object
        let keys = json["keys"].as_array().unwrap(); // Safely extract as an array


        // Define regex pattern for key parsing
        // /164080f7933651c4_2024-04-26--19-07-52--1--qcamera.ts
        let re = Regex::new(r"^([0-9a-z]{16})_([0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)--(.+)$").unwrap();

        for key_value in keys {
            let mut file_name = key_value.as_str().unwrap().to_string(); // Convert to string for independent ownership
            file_name = file_name.strip_prefix("/").unwrap().to_string(); // Strip prefix and convert back to string
    
            match re.captures(&file_name) {
                Some(caps) => {
                    let dongle_id = &caps[1];
                    let timestamp = &caps[2];
                    let segment = &caps[3];
                    let file_type = &caps[4];

                    if file_type.to_string().ends_with(".unlog") {
                        // skip this file 
                        continue

                    } else if file_type.to_string().ends_with("qlog.bz2") {
                        // delete the unlog file from mkv
                        let unlog_file_name = file_name.replace(".bz2", ".unlog");
                        let internal_unlog_url = common::mkv_helpers::get_mkv_file_url(&unlog_file_name).await;
                        tracing::trace!("Deleting: {internal_unlog_url}");
                        let response = client.delete(&internal_unlog_url).send().await.unwrap();
                    }
    
                    let internal_url = common::mkv_helpers::get_mkv_file_url(&file_name).await;
    
                    let result = LogSegmentWorker::perform_later(
                        &app_context,
                        LogSegmentWorkerArgs {
                            internal_file_url: internal_url,
                            dongle_id: dongle_id.to_string(),
                            timestamp: timestamp.to_string(),
                            segment: segment.to_string(),
                            file: file_type.to_string(),
                            create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                        },
                    ).await;
    
                    match result {
                        Ok(_) => tracing::debug!("Queued Worker"),
                        Err(e) => {
                            tracing::error!("Failed to queue worker: {}", e);
                            return Ok(());
                        }
                    }
                },
                None => {
                    tracing::error!("Failed to parse key: {}", file_name);
                    return Ok(());
                }
            }
        }
    
        Ok(())
    }
}
