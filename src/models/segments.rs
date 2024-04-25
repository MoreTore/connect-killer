use sea_orm::{entity::prelude::*, TransactionTrait};
use crate::cereal::legacy_capnp::nav_update::segment;
use async_trait::async_trait;
use chrono::offset::Local;
use loco_rs::{auth::jwt, hash, prelude::*};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::segments::{self, ActiveModel, Entity, Model};

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

pub struct SegmentParams {
    pub canonical_name: String,
    pub url: String,
    pub ulog_url: Option<String>,
    pub qlog_url: String,
    pub qcam_url: Option<String>,
    pub rlog_url: Option<String>,
    pub fcam_url: Option<String>,
    pub dcam_url: Option<String>,
    pub ecam_url: Option<String>,
    pub start_time_utc_millis: i64,
    pub end_time_utc_millis: i64,
    pub end_lng: Option<f64>,
    pub start_lng: Option<f64>,
    pub end_lat: Option<f64>,
    pub start_lat: Option<f64>,
    pub proc_log: i32,
    pub proc_camera: i32,
    pub hgps: bool,
    pub can: bool,
}

impl super::_entities::segments::Model {
    pub async fn add_segment(
        db: &DatabaseConnection, 
        params: &SegmentParams
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;
        if segments::Entity::find()
            .filter(segments::Column::CanonicalName.eq(&params.canonical_name))
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ModelError::EntityAlreadyExists {});
        }
        // xxxxxxxxxxxxxxxx|2024-03-02--19-02-46--NN+
        // [      0       ] [    1   ]  [   2  ]  [3]
        // remove the last part of the canonical name (--0)
        let re = regex::Regex::new(r"^([a-z0-9]{16})\|([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)")
            .expect("Invalid regex");
        let canonical_route;
        match re.captures(&params.canonical_name) {
            Some(caps) => {
                canonical_route = format!("{}|{}--{}",
                    &caps[1], // dongle_id
                    &caps[2], // date
                    &caps[3], // time
                );
            },
            None => canonical_route = "No match found".to_string(),
        }
        
        let segment = segments::ActiveModel {
            
            canonical_name: ActiveValue::Set(params.canonical_name.clone()),
            canonical_route_name: ActiveValue::Set(canonical_route.clone()),
            url: ActiveValue::Set(params.url.clone()),
            ulog_url: ActiveValue::Set(params.ulog_url.clone()),
            qlog_url: ActiveValue::Set(params.qlog_url.clone()),
            rlog_url: ActiveValue::Set(params.rlog_url.clone()),
            fcam_url: ActiveValue::Set(params.fcam_url.clone()),
            dcam_url: ActiveValue::Set(params.dcam_url.clone()),
            ecam_url: ActiveValue::Set(params.ecam_url.clone()),
            start_time_utc_millis: ActiveValue::Set(params.start_time_utc_millis),
            end_time_utc_millis: ActiveValue::Set(params.end_time_utc_millis),
            end_lng: ActiveValue::Set(params.end_lng),
            start_lng: ActiveValue::Set(params.start_lng),
            end_lat: ActiveValue::Set(params.end_lat),
            start_lat: ActiveValue::Set(params.start_lat),
            proc_log: ActiveValue::Set(params.proc_log),
            proc_camera: ActiveValue::Set(params.proc_camera),
            can: ActiveValue::Set(params.can),
            ..Default::default()
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;

        Ok(segment)
    }
}