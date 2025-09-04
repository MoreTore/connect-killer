use std::collections::BTreeMap;
use reqwest::Client;
use regex::Regex;
use serde_json::from_str;
use serde_json::Value;
use loco_rs::prelude::*;
use chrono::{Utc, Duration, NaiveDateTime, ParseError};
use sysinfo::Disks;
use tower_http::trace;
use std::env;
use std::path::Path;

use crate::{
    common::{
        mkv_helpers,
        re::*,
    },
    models::{
        segments::SM,
        devices::DM,
        routes::RM,
    },
};

fn parse_timestamp(timestamp: &str) -> Result<NaiveDateTime, ParseError> {
    NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d--%H-%M-%S")
}

pub struct Cleaner;
#[async_trait]
impl Task for Cleaner {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "cleaner".to_string(),
            detail: "Task generator".to_string(),
        }
    }
    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        println!("Task Cleaner generated");
    
        let client = Client::new();
        let mut retention_minutes = 24 * 60 * 7; // keep for 7 days for debugging

        loop {

            let now: NaiveDateTime = Utc::now().naive_utc();
            let older_than = now - Duration::minutes(retention_minutes);
            tracing::info!("now: {now}, cleaning routes older than: {older_than}");

            // get a list of devices from the database
            let devices = DM::find_all_devices(&ctx.db).await;
            
            for device in devices {
                // get the routes that are older than the retention period
                let routes = RM::find_time_filtered_device_routes(
                    &ctx.db,
                    &device.dongle_id,
                    None,
                    Some(older_than.and_utc().timestamp_millis()),
                    Some(10000)
                ).await?;

                // check the length of each route
                for route in routes {
                    if (route.length < 0.1 && route.hpgps == true) || (route.can == false) {
                        let query = mkv_helpers::list_keys_starting_with(&route.fullname);
                        let response = client.get(&query).send().await.unwrap();
                        if !response.status().is_success() {
                            tracing::info!("Failed to get keys");
                            return Ok(());
                        }
                        let body = response.text().await.unwrap();
                        let json: Value = from_str(&body).unwrap(); // Convert response text into JSON
                        let keys = json["keys"].as_array().unwrap(); // Safely extract as an array
                        for key in keys {
                            let mut file_name = key.as_str().unwrap().to_string(); // Convert to string for independent ownership
                            file_name = file_name.strip_prefix("/").unwrap().to_string(); // Strip prefix and convert back to string
                            tracing::info!("Deleting file: {file_name}");
                            delete_file(&client, &file_name).await;
                        }

                        RM::delete_route(&ctx.db, &route.fullname).await?;
                    }
                }
            }

        }
    }
}

async fn delete_file(client: &Client, file_name: &str) {
    tracing::info!("Deleting file: {file_name}");
    client.delete(&mkv_helpers::get_mkv_file_url(file_name)).send().await.unwrap();
}