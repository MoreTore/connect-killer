use sea_orm::{entity::prelude::*, DeleteResult, TransactionTrait};
use crate::cereal::legacy_capnp::nav_update::segment;
use migration::m20240424_000004_segments::Segments as SegmentFields;
use async_trait::async_trait;
use chrono::offset::Local;
use loco_rs::{auth::jwt, hash, prelude::*};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::segments::{self, ActiveModel, Entity, Model};

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}


#[derive(Default, Serialize, Clone)]
pub struct SegmentParams {
    pub canonical_name: String,
    pub url: String,
    pub ulog_url: String,
    pub qlog_url: String,
    pub qcam_url: String,
    pub rlog_url: String,
    pub fcam_url: String,
    pub dcam_url: String,
    pub ecam_url: String,
    pub start_time_utc_millis: i64,
    pub end_time_utc_millis: i64,
    pub end_lng: Option<f64>,
    pub start_lng: Option<f64>,
    pub end_lat: Option<f64>,
    pub start_lat: Option<f64>,
    pub proc_log: i32,
    pub proc_camera: i32,
    pub hpgps: bool,
    pub can: bool,
}

impl super::_entities::segments::Model {
    // pub async fn add_segment(
    //     db: &DatabaseConnection, 
    //     params: &SegmentParams
    // ) -> ModelResult<Self> {
    //     let txn = db.begin().await?;
    //     if segments::Entity::find()
    //         .filter(segments::Column::CanonicalName.eq(&params.canonical_name))
    //         .one(&txn)
    //         .await?
    //         .is_some()
    //     {
    //         return Err(ModelError::EntityAlreadyExists {});
    //     }
    //     // xxxxxxxxxxxxxxxx|2024-03-02--19-02-46--NN+
    //     // [      0       ] [    1   ]  [   2  ]  [3]
    //     // remove the last part of the canonical name (--0)
    //     let re = regex::Regex::new(r"^([a-z0-9]{16})\|([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)")
    //         .expect("Invalid regex");
    //     let canonical_route;
    //     match re.captures(&params.canonical_name) {
    //         Some(caps) => {
    //             canonical_route = format!("{}|{}--{}",
    //                 &caps[1], // dongle_id
    //                 &caps[2], // date
    //                 &caps[3], // time
    //             );
    //         },
    //         None => canonical_route = "No match found".to_string(),
    //     }
        
    //     let segment = segments::ActiveModel {
    //         canonical_name: ActiveValue::Set(params.canonical_name.clone()),
    //         canonical_route_name: ActiveValue::Set(canonical_route.clone()),
    //         url: ActiveValue::Set(params.url.clone()),
    //         ulog_url: ActiveValue::Set(params.ulog_url.clone()),
    //         qlog_url: ActiveValue::Set(params.qlog_url.clone()),
    //         qcam_url: ActiveValue::Set(params.qcam_url.clone()),
    //         rlog_url: ActiveValue::Set(params.rlog_url.clone()),
    //         fcam_url: ActiveValue::Set(params.fcam_url.clone()),
    //         dcam_url: ActiveValue::Set(params.dcam_url.clone()),
    //         ecam_url: ActiveValue::Set(params.ecam_url.clone()),
    //         start_time_utc_millis: ActiveValue::Set(params.start_time_utc_millis),
    //         end_time_utc_millis: ActiveValue::Set(params.end_time_utc_millis),
    //         end_lng: ActiveValue::Set(params.end_lng),
    //         start_lng: ActiveValue::Set(params.start_lng),
    //         end_lat: ActiveValue::Set(params.end_lat),
    //         start_lat: ActiveValue::Set(params.start_lat),
    //         proc_log: ActiveValue::Set(params.proc_log),
    //         proc_camera: ActiveValue::Set(params.proc_camera),
    //         can: ActiveValue::Set(params.can),
    //         ..Default::default()
    //     }
    //     .insert(&txn)
    //     .await?;

    //     txn.commit().await?;

    //     Ok(segment)
    // }

    pub async fn add_segment_self(
        self,
        db: &DatabaseConnection,
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;
        if segments::Entity::find()
            .filter(segments::Column::CanonicalName.eq(&self.canonical_name))
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
        match re.captures(&self.canonical_name) {
            Some(caps) => {
                canonical_route = format!("{}|{}--{}",
                    &caps[1], // dongle_id
                    &caps[2], // date
                    &caps[3], // time
                );
            },
            None => canonical_route = "No match found".to_string(),
        }
        let active_model = self.clone().into_active_model();
        active_model.insert(&txn)
        .await?;
        txn.commit().await?;
        Ok(self)
        
    }

    pub async fn find_by_segment(
        db: &DatabaseConnection,
        canonical_name: &String,
    ) -> ModelResult<Model> {
        let segment = segments::Entity::find()
            .filter(segments::Column::CanonicalName.eq(canonical_name))
            .one(db)
            .await?;
        segment.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_segments_by_route(
        db: &DatabaseConnection,
        canonical_route_name: &str,
    ) -> ModelResult<Vec<Model>> {
        let segments = segments::Entity::find()
            .filter(segments::Column::CanonicalRouteName.eq(canonical_route_name))
            .all(db)
            .await?;
        Ok(segments)
    }

    pub async fn find_all_segments(
        db: &DatabaseConnection,
    ) -> ModelResult<Vec<Model>> {
        let segments = segments::Entity::find()
            .all(db)
            .await?;
        Ok(segments)
    }

    pub async fn delete_segment(
        db: &DatabaseConnection,
        canonical_name: &str,
    ) -> ModelResult<DeleteResult> {
        Ok(segments::Entity::delete_by_id(canonical_name).exec(db).await?)    
        
    }

    pub async fn delete_segments(
        db: &DatabaseConnection,
        canonical_route_name: &str,
    ) -> ModelResult<DeleteResult> {
        Ok(segments::Entity::delete_many()
            .filter(segments::Column::CanonicalRouteName.eq(canonical_route_name))
            //.filter(segments::Column::CanonicalName.contains(canonical_route_name))
            .exec(db)
            .await?)
    }
}

impl super::_entities::segments::ActiveModel {
    pub async fn update_field(
        mut self,
        db: &DatabaseConnection,
        field: &SegmentFields,
        value: String,
    ) -> ModelResult<Model> {
        macro_rules! parse_value {
            ($value:expr, $type:ty) => {
                $value.parse::<$type>().map_err(|e| {
                    tracing::error!("Failed to parse value: '{}', error: {}", $value, e);
                    ModelError::EntityNotFound
                })?
            };
        }

        match field {                
            SegmentFields::Url => self.url = ActiveValue::Set(value),
            SegmentFields::UlogUrl => self.ulog_url = ActiveValue::Set(value),
            SegmentFields::QlogUrl => self.qlog_url = ActiveValue::Set(value),
            SegmentFields::QcamUrl => self.qcam_url = ActiveValue::Set(value),
            SegmentFields::RlogUrl => self.rlog_url = ActiveValue::Set(value),
            SegmentFields::FcamUrl => self.fcam_url = ActiveValue::Set(value),
            SegmentFields::DcamUrl => self.dcam_url = ActiveValue::Set(value),
            SegmentFields::EcamUrl => self.ecam_url = ActiveValue::Set(value),
            SegmentFields::GitBranch => self.git_branch = ActiveValue::Set(Some(value)),

            SegmentFields::StartTimeUtcMillis => self.start_time_utc_millis = ActiveValue::Set(parse_value!(value, i64)),
            SegmentFields::EndTimeUtcMillis => self.end_time_utc_millis = ActiveValue::Set(parse_value!(value, i64)),

            SegmentFields::StartLng => self.start_lng = ActiveValue::Set(parse_value!(value, f64)),
            SegmentFields::EndLng => self.end_lng = ActiveValue::Set(parse_value!(value, f64)),
            SegmentFields::StartLat => self.start_lat = ActiveValue::Set(parse_value!(value, f64)),
            SegmentFields::EndLat => self.end_lat = ActiveValue::Set(parse_value!(value, f64)),

            SegmentFields::CreateTime => self.create_time = ActiveValue::Set(parse_value!(value, i64)),

            SegmentFields::ProcLog => self.proc_log = ActiveValue::Set(parse_value!(value, i32)),
            SegmentFields::ProcCamera => self.proc_camera = ActiveValue::Set(parse_value!(value, i32)),



            SegmentFields::Number => self.number = ActiveValue::Set(parse_value!(value, i16)),

            SegmentFields::Hpgps => self.hpgps = ActiveValue::Set(parse_value!(value, bool)),
            SegmentFields::Can => self.can = ActiveValue::Set(parse_value!(value, bool)),
            SegmentFields::Passive => self.passive = ActiveValue::Set(Some(parse_value!(value, bool))),
            
            SegmentFields::CanonicalRouteName => (),
            SegmentFields::CanonicalName => (),

            SegmentFields::Table => return Err(ModelError::EntityNotFound),
        }
        Ok(self.update(db).await?)
    }
}