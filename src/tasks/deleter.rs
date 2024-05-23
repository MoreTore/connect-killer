use std::collections::BTreeMap;

use loco_rs::prelude::*;

use crate::models::_entities::devices;
use crate::models::_entities::users;
use crate::models::_entities::segments;
use crate::models::_entities::routes;
use crate::models::_entities::bootlogs;


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
        let re_boot_log = regex::Regex::new(r"^([0-9a-z]{16})_([0-9a-z]{8}--[0-9a-z]{10}.bz2$)").unwrap();
        let re = Regex::new(r"^([0-9a-z]{16})_([0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)--(.+)$").unwrap();

        let client = Client::new();
        // Get all keys from the MKV server
        let query = common::mkv_helpers::list_keys_starting_with("");
        let response = client.get(&query).send().await.unwrap();

        if !response.status().is_success() {
            tracing::info!("Failed to get keys");
            return Ok(());
        }

        let body = response.text().await.unwrap();
        let json: Value = from_str(&body).unwrap(); // Convert response text into JSON

        // Extract keys from the JSON object
        let keys = json["keys"].as_array().unwrap(); // Safely extract as an array

        

        for key in keys {
            let mut file_name = key_value.as_str().unwrap().to_string(); // Convert to string for independent ownership
            file_name = file_name.strip_prefix("/").unwrap().to_string(); // Strip prefix and convert back to string
            match re.captures(&file_name) {
                Some(caps) => {
                    let dongle_id = &caps[1];
                    let timestamp = &caps[2];
                    let segment = &caps[3];
                    let file_type = &caps[4];
                    match devices::Model::find_device(&ctx.db, &dongle_id) {
                        Ok(_) => (),
                        Err(model_error) => match model_error {
                            ModelError::EntityNotFound => {

                            }
                            _ => {
                                tracing::error(model_error.to_string()),
                            }
                        }
                    }
                }
                None => {
                    tracing::error("Unkown file or bootlog in kv store. Deleting it!");
                    let internal_file_url = common::mkv_helpers::get_mkv_file_url(&file_name);
                    //let response = client.delete(&internal_file_url).send().await.unwrap();
                }
            }

        }

        segment_models = segments::Model::find_all_segments(&ctx.db).await?;
        route_models = routes::Model::find_all_routes(&ctx.db).await?;
        Ok(())
    }
}


async fn delete_device_routes(ctx: &AppContext, dongle_id: &str) {
    
}