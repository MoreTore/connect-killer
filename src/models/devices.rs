use sea_orm::entity::prelude::*;
pub use super::_entities::devices::{self, ActiveModel, Entity, Model};
use async_trait::async_trait;
use chrono::offset::Local;
use loco_rs::{auth::jwt, hash, prelude::*};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct DeviceRegistrationParams {
    pub id: i32,
    pub dongle_id: String,
    pub serial_number: String,
    pub user_id: i32,
    pub sim_id: String,
}

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

// impl super::_entities::users::Model {
//     /// Find all devices associated with a user
//     /// 
//     /// 
//     /// Returns a list of devices associated with the user.
//     /// Can be empty if the user has no devices
//     // pub async fn find_devices(&self, db: &DatabaseConnection) -> Vec<devices::Model> {
//     //     devices::Entity::find()
//     //         .filter(devices::Column::UserId.eq(self.id))
//     //         .all(db)
//     //         .await
//     //         .expect("Database query failed")
//     // }

//     // pub async fn add_device(
//     //     &self, 
//     //     db: &DatabaseConnection, 
//     //     params: &DeviceRegistrationParams) -> ModelResult<Model> {
//     //     let txn = db.begin().await?;

//     //     let device = devices::ActiveModel {
//     //         dongle_id: ActiveValue::Set(params.dongle_id.to_string()),   
//     //         user_id: ActiveValue::Set(self.id),
//     //         ..Default::default()
//     //     }
//     //     .insert(&txn)
//     //     .await?;

//     //     txn.commit().await?;

//     //     Ok(device)
//     // }

// }