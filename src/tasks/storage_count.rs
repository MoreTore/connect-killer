use std::collections::BTreeMap;
use reqwest::Client;
use serde_json::{Value, from_str};
use regex::Regex;
use std::collections::HashMap;
use loco_rs::prelude::*;

use crate::common::mkv_helpers;
use crate::common::re::*;
use crate::models::devices;

pub struct StorageCount;
#[async_trait]
impl Task for StorageCount {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "storage_count".to_string(),
            detail: "Task generator".to_string(),
        }
    }
    async fn run(&self, ctx: &AppContext, _vars: &BTreeMap<String, String>) -> Result<()> {
        println!("Task StorageCount generated");
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

        let mut total_bytes: i128 = 0;
        let mut largest_files: Vec<(String, i128)> = Vec::new();
        let mut storage_by_dongle: HashMap<String, i128> = HashMap::new();
        let mut unmatched_files_total: i128 = 0;

        let segment_file_regex_string = format!(
            r"^({DONGLE_ID})_({ROUTE_NAME})--({NUMBER})--({ALLOWED_FILENAME}$)"
        );
        let segment_file_regex = Regex::new(&segment_file_regex_string).unwrap();

        // Extract keys from the JSON object
        let keys = json["keys"].as_array().unwrap(); // Safely extract as an array
        for key_value in keys {
            let file_name = key_value.as_str().unwrap().trim_start_matches('/').to_string(); // Convert to string for independent ownership
            let file_url = mkv_helpers::get_mkv_file_url(&file_name);

            // Make a HEAD request to get the file's metadata, including the size
            let head_response = client.head(&file_url).send().await.unwrap();

            if head_response.status().is_success() {
                if let Some(content_length) = head_response.headers().get(reqwest::header::CONTENT_LENGTH) {
                    if let Ok(content_length_str) = content_length.to_str() {
                        if let Ok(content_length_val) = content_length_str.parse::<i128>() {
                            total_bytes += content_length_val;

                            if let Some(captures) = segment_file_regex.captures(&file_name) {
                                let dongle_id = captures.get(1).map_or("", |m| m.as_str()).to_string();

                                // Accumulate storage used by each unique DONGLE_ID
                                let entry = storage_by_dongle.entry(dongle_id.clone()).or_insert(0);
                                *entry += content_length_val;
                            } else {
                                // Accumulate storage for unmatched files
                                unmatched_files_total += content_length_val;
                            }

                            // Insert the file into the top 100 largest files
                            largest_files.push((file_name.clone(), content_length_val));
                            largest_files.sort_by(|a, b| b.1.cmp(&a.1)); // Sort in descending order by size

                            if largest_files.len() > 100 {
                                largest_files.pop(); // Keep only the top 100
                            }
                        }
                    }
                }
            } else {
                tracing::info!("Failed to get file size for {}", file_name);
            }
        }

        let total_gb = total_bytes as f64 / 1_000_000_000.0;
        let unmatched_gb = unmatched_files_total as f64 / 1_000_000_000.0;

        println!("Total storage used: {:.2} GB", total_gb);
        // println!("Top 100 largest files:");
        // for (file_name, size) in &largest_files {
        //     tracing::info!("File: {}, Size: {:.2} GB", file_name, *size as f64 / 1_000_000_000.0);
        // }

        println!("Storage used by each DONGLE_ID (in GB):");
        for (dongle_id, storage) in &storage_by_dongle {
            let storage_gb = *storage as f64 / 1_000_000_000.0;
            if let Ok(device_model) = devices::DM::find_device(&ctx.db, dongle_id).await {
                let mut active_device_model = device_model.into_active_model();
                active_device_model.server_storage = ActiveValue::Set(*storage as i64);
                if active_device_model.update(&ctx.db).await.is_err() {
                    tracing::error!("Failed to update server_storage for {}", dongle_id);
                }
            }
            println!("DONGLE_ID: {}, Storage Used: {:.2} GB", dongle_id, storage_gb);
        }

        println!("Total storage used by unmatched files: {:.2} GB", unmatched_gb);

        Ok(())
    }
}