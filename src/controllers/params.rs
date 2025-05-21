use loco_rs::prelude::*;
use axum::extract::Query;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
pub struct ParamsQuery {
    params: Option<String>,
}

pub async fn list_or_get_params(
    _auth: crate::middleware::auth::MyJWT,
    State(_ctx): State<AppContext>,
    Query(query): Query<ParamsQuery>,
) -> Result<Response> {
    if let Some(params_str) = query.params {
        let param_list: HashSet<_> = params_str.split(',').map(|s| s.trim().to_string()).collect();
        let mut result = serde_json::Map::new();
        for p in param_list {
            let try_paths = [
                format!("/params/{}.json", p),
                format!("/params/{}", p),
            ];
            if let Some(file) = try_paths.iter().find_map(|path| std::fs::read(path).ok()) {
                if let Ok(val) = String::from_utf8(file) {
                    result.insert(p, serde_json::from_str(&val).unwrap_or(serde_json::Value::String(val)));
                }
            }
        }
        let json = serde_json::to_string(&result)?;
        return Ok(Response::builder()
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(json))
            .unwrap());
    }
    let params = std::fs::read_dir("/params")?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name()?.to_str()?.to_string();
                if let Some(stripped) = file_name.strip_suffix(".json") {
                    Some(stripped.to_string())
                } else {
                    Some(file_name)
                }
            } else {
                None
            }
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let json = serde_json::to_string(&params)?;
    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(json))
        .unwrap())
}


pub async fn get_params_metrics(
    _auth: crate::middleware::auth::MyJWT,
    State(_ctx): State<AppContext>,
    Query(query): Query<ParamsQuery>,
) -> Result<Response> {
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::Path;

    // Parse the params from the query string
    let param_set: Option<HashSet<String>> = query.params.as_ref().map(|params_str| {
        params_str.split(',').map(|s| s.trim().to_string()).collect()
    });

    // Map: param_name -> value -> count
    let mut result: HashMap<String, HashMap<String, usize>> = HashMap::new();

    // List all device param files in /params/devices/
    let devices_dir = Path::new("/params/devices");
    if let Ok(entries) = fs::read_dir(devices_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&data) {
                        let keys: Box<dyn Iterator<Item = &String>> = if let Some(ref param_set) = param_set {
                            Box::new(param_set.iter())
                        } else {
                            Box::new(json.keys())
                        };
                        for param in keys {
                            if let Some(val) = json.get(param) {
                                let val_str = val.as_str().map(|s| s.to_string()).unwrap_or_else(|| val.to_string());
                                let entry = result.entry(param.clone()).or_insert_with(HashMap::new);
                                *entry.entry(val_str).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    let json = serde_json::to_string(&result)?;
    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(json))
        .unwrap())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("params")
        .add("/", get(list_or_get_params))
        .add("/metrics", get(get_params_metrics))
}
