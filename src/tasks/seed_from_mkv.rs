use rand::rngs::StdRng;
use rand::{SeedableRng, Rng};
use rand::seq::SliceRandom;
use time::serde::timestamp; use std::env::vars;
// For the `shuffle` method
use std::thread;
use std::time::Duration;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::{Value, from_str};
use crate::{common, workers::{bootlog_parser::{BootlogParserWorker, BootlogParserWorkerArgs}, log_parser::{LogSegmentWorker, LogSegmentWorkerArgs}}};
use loco_rs::prelude::*;
use crate::common::re::*;
pub struct SeedFromMkv;

#[async_trait]
impl Task for SeedFromMkv {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "seed_from_mkv".to_string(),
            detail: "Task generator".to_string(),
        }
    }

    async fn run(&self, app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        println!("Task SeedFromMkv generated");
        let dongle_id_filter = vars
            .cli_arg("dongle_id")
            .ok();
        let timestamp_filter = vars
            .cli_arg("timestamp")
            .ok();

        let client = Client::new();
        // Get all keys from the MKV server
        let query = if let Some(dongle_id_filter) = dongle_id_filter {
            common::mkv_helpers::list_keys_starting_with(&format!("{}", dongle_id_filter))
        } else {
            common::mkv_helpers::list_keys_starting_with("")
        };
        //let query = common::mkv_helpers::list_keys_starting_with("");
        let response = client.get(&query).send().await.unwrap();

        if !response.status().is_success() {
            tracing::info!("Failed to get keys");
            return Ok(());
        }

        let body = response.text().await.unwrap();
        let json: Value = from_str(&body).unwrap(); // Convert response text into JSON

        // Extract keys from the JSON object
        let mut keys= json["keys"].as_array().unwrap().clone(); // Safely extract as an array

        let mut rng = StdRng::from_entropy();
        keys.shuffle(&mut rng);
        // Define regex pattern for key parsing
        let segment_file_regex_string = format!(
            r"^({DONGLE_ID})_({ROUTE_NAME})--({NUMBER})--({ALLOWED_FILENAME}$)"
        );
        let re = regex::Regex::new(&segment_file_regex_string).unwrap();
        for key_value in keys {
            let file_name = key_value.as_str().unwrap().trim_start_matches('/').to_string(); // Convert to string for independent ownership
    
            match re.captures(&file_name) {
                Some(caps) => {
                    let dongle_id = caps[1].to_string(); 
                    let timestamp = caps[2].to_string(); // DateTime or monotonic--uid
                    let segment = caps[3].to_string();
                    let file_type = caps[4].to_string();

                    if (file_type != "qlog.bz2") && (file_type != "qlog.zst") {
                        continue;
                    }

                    if timestamp_filter.map_or(false, |filter| filter != &timestamp) {
                        continue;
                    }

                    if  file_type.to_string().ends_with(".unlog") || 
                        file_type.to_string().ends_with("sprite.jpg") ||
                        file_type.to_string().ends_with("coords.json") {
                        // skip this file 
                        continue
                    
                    } else if (file_type.to_string().ends_with("qlog.bz2")) || (file_type.to_string().ends_with("qlog.zst")) {
                        // delete the unlog file from mkv
                        let unlog_file_name = file_name.replace(".bz2", ".unlog").replace(".zst", ".unlog");
                        let internal_unlog_url = common::mkv_helpers::get_mkv_file_url(&unlog_file_name);
                        tracing::trace!("Deleting: {internal_unlog_url}");
                        let response = client.delete(&internal_unlog_url).send().await.unwrap();
                        // delete the sprite file from mkv
                        let sprite_file_name = file_name.replace("qlog.bz2", "sprite.jpg").replace("qlog.zst", "sprite.jpg");
                        let internal_sprite_url = common::mkv_helpers::get_mkv_file_url(&sprite_file_name);
                        tracing::trace!("Deleting: {internal_sprite_url}");
                        let response = client.delete(&internal_sprite_url).send().await.unwrap();
                        let coords_file_name = file_name.replace("qlog.bz2", "coords.json").replace("qlog.zst", "coords.json");
                        let internal_coords_url = common::mkv_helpers::get_mkv_file_url(&coords_file_name);
                        tracing::trace!("Deleting: {internal_coords_url}");
                        let response = client.delete(&internal_coords_url).send().await.unwrap();
                    }
    
                    let internal_url = common::mkv_helpers::get_mkv_file_url(&file_name);
                    
                    let result = LogSegmentWorker::perform_later(
                        &app_context,
                        LogSegmentWorkerArgs {
                            internal_file_url: internal_url,
                            dongle_id: dongle_id.to_string(),
                            timestamp: timestamp.to_string(),
                            segment: segment.to_string(),
                            file: file_type.to_string(),
                            create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                        },
                    ).await;
    
                    match result {
                        Ok(_) => tracing::info!("Queued Worker"),
                        Err(e) => {
                            tracing::error!("Failed to queue worker: {}", e);
                            return Ok(());
                        }
                    }
                },
                None => {
                    continue;
                    
                    
                    let re_boot_log = regex::Regex::new(r"^([0-9a-z]{16})_([0-9a-z]{8}--[0-9a-z]{10}.bz2$)").unwrap();
                    match re_boot_log.captures(&file_name) {
                        Some(caps) => {
                            let dongle_id = &caps[1];
                            let file = &caps[2];
                            let internal_file_url = common::mkv_helpers::get_mkv_file_url(&file_name);
                            let unlog_internal_file_url = internal_file_url.replace(".bz2", ".unlog");
                            let _response = client.delete(&unlog_internal_file_url).send().await.unwrap();
                            let _result = BootlogParserWorker::perform_later(&app_context, 
                                BootlogParserWorkerArgs {
                                    internal_file_url: internal_file_url.clone(),
                                    dongle_id: dongle_id.into(),
                                    file_name: file.into(),
                                    create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                                },
                              ).await;
                        }
                    
                        
                        None => {
                            tracing::error!("Failed to parse key: {}", file_name);
                            continue;
                        }
                    }
                }
            }
        }
        loop {
            let sec = Duration::from_secs(1);
            thread::sleep(sec);
            println!("waiting..");
        }
        Ok(())
    }
}

pub async fn boot_logs() {

}
