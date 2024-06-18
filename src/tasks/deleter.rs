use std::collections::BTreeMap;
use reqwest::Client;
use regex::Regex;
use serde_json::from_str;
use serde_json::Value;
use loco_rs::prelude::*;
use chrono::{Utc, Duration, NaiveDateTime, ParseError};

use crate::{models::_entities::{
    segments,
    },
    common::mkv_helpers,
};

fn parse_timestamp(timestamp: &str) -> Result<NaiveDateTime, ParseError> {
    NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d--%H-%M-%S")
}

pub struct Deleter;
#[async_trait]
impl Task for Deleter {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "deleter".to_string(),
            detail: "Task generator".to_string(),
        }
    }
    async fn run(&self, ctx: &AppContext, _vars: &BTreeMap<String, String>) -> Result<()> {
        println!("Task Deleter generated");
        let _re_boot_log = regex::Regex::new(r"^([0-9a-z]{16})_([0-9a-z]{8}--[0-9a-z]{10}.bz2$)").unwrap();
        let re = Regex::new(r"^([0-9a-z]{16})_([0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2}|[0-9a-f]{8}--[0-9a-f]{10})--([0-9]+)--(.+)$").unwrap();

        let client = Client::new();
        // Get all keys from the MKV server
        let query = mkv_helpers::list_keys_starting_with("");
        let response = client.get(&query).send().await.unwrap();

        if !response.status().is_success() {
            tracing::info!("Failed to get keys");
            return Ok(());
        }

        let body = response.text().await.unwrap();
        let json: Value = from_str(&body).unwrap(); // Convert response text into JSON

        // Extract keys from the JSON object
        let keys = json["keys"].as_array().unwrap(); // Safely extract as an array
        // TODO: Refactor to not load the whole response in ram at once as it could get large.
        let now: NaiveDateTime = Utc::now().naive_utc();
        let older_than = now - Duration::days(1);
        tracing::info!("now: {now}, deleting files older than: {older_than}");
        for key in keys {
            let mut file_name = key.as_str().unwrap().to_string(); // Convert to string for independent ownership
            file_name = file_name.strip_prefix("/").unwrap().to_string(); // Strip prefix and convert back to string
            match re.captures(&file_name) {
                Some(caps) => {
                    let dongle_id = &caps[1];
                    let timestamp = &caps[2];
                    let segment = &caps[3];
                    let _file_type = &caps[4];
                    match segments::Model::find_by_segment(&ctx.db, &format!("{dongle_id}|{timestamp}--{segment}")).await {
                        Ok(segment) => {
                            let mut deleted = false;
                            // if let Ok(derived_dt) = parse_timestamp(timestamp) { // Try using file name timestamp first
                            //     if derived_dt <= older_than {
                            //         delete_file(&client, &file_name).await;
                            //         deleted = true;
                            //     }
                            // };
                            if segment.updated_at <= older_than && !deleted { // Fallback to updated_at
                                delete_file(&client, &file_name).await;
                            }
                        },
                        Err(_e) => {
                            tracing::error!("No segment found for file: {file_name}. ");
                            if let Ok(derived_dt) = parse_timestamp(timestamp) {
                                if derived_dt <= older_than {
                                    delete_file(&client, &file_name).await;
                                }
                            };
                        }   
                    }
                }
                None => {
                    tracing::error!("Unkown file or bootlog in kv store. Deleting it!");
                    delete_file(&client, &file_name).await;
                }
            }

        }
        Ok(())
    }
}

async fn delete_file(client: &Client, file_name: &str) {
    tracing::info!("Deleting file: {file_name}");
    client.delete(&mkv_helpers::get_mkv_file_url(file_name)).send().await.unwrap();
}