use chrono::prelude::Utc;
use loco_rs::prelude::*;
use serde::Serialize;
use sea_orm::{entity::prelude::*, DeleteResult, TransactionTrait, QueryOrder};
pub use super::_entities::segments::{self, ActiveModel, Entity, Model as SM, Column};


#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert {
            this.created_at = ActiveValue::Set(Utc::now().naive_utc());
            this.updated_at = ActiveValue::Set(Utc::now().naive_utc());
            Ok(this)
        } else {
            // update time
            this.updated_at = ActiveValue::Set(Utc::now().naive_utc());
            Ok(this)
        }
    }
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
    pub proclog: i32,
    pub proccamera: i32,
    pub hpgps: bool,
    pub can: bool,
}

impl SM {

    pub async fn add_segment_self(
        self,
        db: &DatabaseConnection,
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;
        if Entity::find()
            .filter(Column::CanonicalName.eq(&self.canonical_name))
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ModelError::EntityAlreadyExists {});
        }
        // xxxxxxxxxxxxxxxx|2024-03-02--19-02-46--NN+
        // [      0       ] [    1   ]  [   2  ]  [3]
        // remove the last part of the canonical name (--0)
        // let re = regex::Regex::new(r"^([a-z0-9]{16})\|([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)")
        //     .expect("Invalid regex");
        // let canonical_route;
        // match re.captures(&self.canonical_name) {
        //     Some(caps) => {
        //         canonical_route = format!("{}|{}--{}",
        //             &caps[1], // dongle_id
        //             &caps[2], // date
        //             &caps[3], // time
        //         );
        //     },
        //     None => canonical_route = "No match found".to_string(),
        // }
        let active_model = self.clone().into_active_model();
        active_model.insert(&txn)
        .await?;
        txn.commit().await?;
        Ok(self)
        
    }

    pub async fn find_one(
        db: &DatabaseConnection,
        canonical_name: &String,
    ) -> ModelResult<SM> {
        let segment = Entity::find()
            .filter(Column::CanonicalName.eq(canonical_name))
            .one(db)
            .await?;
        segment.ok_or_else(|| ModelError::EntityNotFound)
    }
    /// Sorted by segment number ascending
    pub async fn find_segments_by_route(
        db: &DatabaseConnection,
        canonical_route_name: &str,
    ) -> ModelResult<Vec<SM>> {
        let segments = Entity::find()
            .filter(Column::CanonicalRouteName.eq(canonical_route_name))
            .order_by_asc(Column::Number)
            .all(db)
            .await?;
        Ok(segments)
    }
    /// Sorted by segment number ascending
    pub async fn find_time_filtered_device_segments(
        db: &DatabaseConnection,
        dongle_id: &str,
        from: i64,
        to: i64,
    ) -> ModelResult<Vec<SM>> {
        let segments = Entity::find()
            .filter(Column::CanonicalRouteName.starts_with(dongle_id))
            .filter(Column::StartTimeUtcMillis.between(from, to))
            .order_by_asc(Column::Number)
            .all(db)
            .await?;
        Ok(segments)
    }

    /// This probably should never be used at scale
    pub async fn find_all_segments(
        db: &DatabaseConnection,
    ) -> ModelResult<Vec<SM>> {
        let segments = Entity::find()
            .all(db)
            .await?;
        Ok(segments)
    }

    pub async fn delete_segment(
        db: &DatabaseConnection,
        canonical_name: &str,
    ) -> ModelResult<DeleteResult> {
        Ok(Entity::delete_by_id(canonical_name).exec(db).await?)    
        
    }

    pub async fn delete_segments(
        db: &DatabaseConnection,
        canonical_route_name: &str,
    ) -> ModelResult<DeleteResult> {
        Ok(Entity::delete_many()
            .filter(Column::CanonicalRouteName.eq(canonical_route_name))
            //.filter(Column::CanonicalName.contains(canonical_route_name))
            .exec(db)
            .await?)
    }
}

impl ActiveModel {
    
}