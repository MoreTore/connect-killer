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
    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        println!("Task StorageCount generated");
        let client = Client::new();
        
        // Hex characters for prefix chunking
        let hex_chars = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", 
                        "a", "b", "c", "d", "e", "f"];
        
        let mut total_bytes: i128 = 0;
        let mut largest_files: Vec<(String, i128)> = Vec::new();
        let mut storage_by_dongle: HashMap<String, i128> = HashMap::new();
        let mut unmatched_files_total: i128 = 0;

        let segment_file_regex_string = format!(
            r"^({DONGLE_ID})_({ROUTE_NAME})--({NUMBER})--({ALLOWED_FILENAME}$)"
        );
        let segment_file_regex = Regex::new(&segment_file_regex_string).unwrap();

        // Process each hex prefix sequentially
        for prefix in hex_chars.iter() {
            let query = mkv_helpers::list_keys_starting_with(prefix);
            let response = client.get(&query).send().await.unwrap();

            if !response.status().is_success() {
                tracing::info!("Failed to get keys for prefix {}", prefix);
                continue;
            }

            let body = response.text().await.unwrap();
            let json: Value = from_str(&body).unwrap();

            if let Some(keys) = json["keys"].as_array() {
                for key_value in keys {
                    let file_name = key_value.as_str().unwrap().trim_start_matches('/').to_string();
                    let file_url = mkv_helpers::get_mkv_file_url(&file_name);

                    // Make HEAD request to get file size
                    let head_response = client.head(&file_url).send().await.unwrap();

                    if head_response.status().is_success() {
                        if let Some(content_length) = head_response.headers().get(reqwest::header::CONTENT_LENGTH) {
                            if let Ok(content_length_str) = content_length.to_str() {
                                if let Ok(content_length_val) = content_length_str.parse::<i128>() {
                                    total_bytes += content_length_val;

                                    if let Some(captures) = segment_file_regex.captures(&file_name) {
                                        let dongle_id = captures.get(1).map_or("", |m| m.as_str()).to_string();
                                        storage_by_dongle.entry(dongle_id.clone())
                                            .and_modify(|e| *e += content_length_val)
                                            .or_insert(content_length_val);
                                    } else {
                                        unmatched_files_total += content_length_val;
                                    }

                                    // Maintain top 100 largest files
                                    largest_files.push((file_name.clone(), content_length_val));
                                    largest_files.sort_by(|a, b| b.1.cmp(&a.1));
                                    if largest_files.len() > 100 {
                                        largest_files.pop();
                                    }
                                }
                            }
                        }
                    } else {
                        tracing::info!("Failed to get file size for {}", file_name);
                    }
                }
            }
        }

        let total_gb = total_bytes as f64 / 1_000_000_000.0;
        let unmatched_gb = unmatched_files_total as f64 / 1_000_000_000.0;

        println!("Total storage used: {:.2} GB", total_gb);
        println!("Storage used by each DONGLE_ID (in GB):");

        // Update database with storage information
        for (dongle_id, storage) in &storage_by_dongle {
            let storage_gb = *storage as f64 / 1_000_000_000.0;
            if let Ok(device_model) = devices::DM::find_device(&ctx.db, dongle_id).await {
                let mut active_device_model = device_model.into_active_model();
                active_device_model.server_storage = ActiveValue::Set(*storage as i64);
                if let Err(e) = active_device_model.update(&ctx.db).await {
                    tracing::error!("Failed to update {}: {}", dongle_id, e);
                }
            }
            println!("{}: {:.2} GB", dongle_id, storage_gb);
        }

        println!("Unmatched files storage: {:.2} GB", unmatched_gb);
        Ok(())
    }
}