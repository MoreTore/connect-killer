use std::f64::consts::PI;
use std::hash::Hasher;
use std::hash::Hash;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use bytes::Bytes;
use std::path::Path;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use tracing;

#[derive(Serialize, Deserialize)]
pub struct ValueCounts(pub HashMap<String, u64>);

pub static PARAM_COUNTS: Lazy<DashMap<String, DashMap<String, u64>>> = Lazy::new(DashMap::new);
pub static DEVICE_PARAMS: Lazy<DashMap<String, DashMap<String, String>>> = Lazy::new(DashMap::new);

pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371e3; // Earth's radius in meters
    let phi1 = lat1 * PI / 180.0;
    let phi2 = lat2 * PI / 180.0;
    let delta_phi = (lat2 - lat1) * PI / 180.0;
    let delta_lambda = (lon2 - lon1) * PI / 180.0;

    let a = (delta_phi / 2.0).sin().powi(2)
          + phi1.cos() * phi2.cos() * (delta_lambda / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    r * c // Distance in meters
}

pub fn calculate_advisory_lock_key(canonical_name: &str) -> u32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical_name.hash(&mut hasher);
    // Get the lower 32 bits of the hash
    hasher.finish() as u32
}

pub fn increment_param_value(param_name: &str, value: &str) {
    let value_counts = PARAM_COUNTS
        .entry(param_name.to_string())
        .or_insert_with(DashMap::new);
    value_counts
        .entry(value.to_string())
        .and_modify(|count| *count += 1)
        .or_insert(1);
}



pub async fn persist_param_value_counts(
    storage: &loco_rs::storage::Storage,
) -> loco_rs::storage::StorageResult<()> {
    use futures::future::try_join_all;

    let mut tasks = Vec::new();

    for param_entry in PARAM_COUNTS.iter() {
        let param_name = param_entry.key().clone();
        let value_counts_map = param_entry.value();

        // Collect current in-memory counts for this param
        let new_counts: HashMap<String, u64> = value_counts_map
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect();

        let storage = storage.clone();
        tasks.push(async move {
            let formatted_path = format!("params/{}.json", param_name);
            let path = Path::new(&formatted_path);
            // Try to load existing counts
            let mut merged = if let Ok(bytes) = storage.download::<Vec<u8>>(path).await {
                serde_json::from_slice::<ValueCounts>(&bytes).unwrap_or(ValueCounts(HashMap::new())).0
            } else {
                tracing::info!("No existing counts for '{}', creating new file", param_name);
                HashMap::new()
            };
            // Merge new counts
            for (k, v) in &new_counts {
                *merged.entry(k.clone()).or_insert(0) += v;
            }
            // If more than 100 unique entries, keep only the 100 highest counts
            if merged.len() > 100 {
                let mut entries: Vec<_> = merged.into_iter().collect();
                entries.sort_by(|a, b| b.1.cmp(&a.1)); // descending by count
                entries.truncate(100);
                merged = entries.into_iter().collect();
            }
            // Write back
            let data = serde_json::to_vec(&ValueCounts(merged.clone())).unwrap();
            storage.upload(path, &Bytes::from(data)).await?;
            Ok::<(), loco_rs::storage::StorageError>(())
        });
    }

    // Await all persist tasks
    try_join_all(tasks).await?;

    tracing::info!("Clearing in-memory PARAM_COUNTS after persisting.");
    PARAM_COUNTS.clear();

    Ok(())
}


pub fn save_device_param(
    device_id: &str,
    param_name: &str,
    value: &str,
) {
    let device_map = DEVICE_PARAMS
        .entry(device_id.to_string())
        .or_insert_with(DashMap::new);
    device_map.insert(param_name.to_string(), value.to_string());
}

pub async fn persist_device_params(
    storage: &loco_rs::storage::Storage,
) -> loco_rs::storage::StorageResult<()> {
    use futures::future::try_join_all;
    use std::collections::HashMap;
    use std::path::Path;
    use bytes::Bytes;

    let mut tasks = Vec::new();
    for entry in DEVICE_PARAMS.iter() {
        let device_id = entry.key().clone();
        let param_map: HashMap<String, String> = entry
            .value()
            .iter()
            .map(|kv| (kv.key().clone(), kv.value().clone()))
            .collect();
        let storage = storage.clone();
        tasks.push(async move {
            let file_path = format!("params/devices/{}.json", device_id);
            let path = Path::new(&file_path);
            let data = serde_json::to_vec_pretty(&param_map).unwrap();
            storage.upload(path, &Bytes::from(data)).await?;
            Ok::<(), loco_rs::storage::StorageError>(())
        });
    }
    try_join_all(tasks).await?;
    Ok(())
}