use sea_orm::entity::prelude::*;
pub use super::_entities::devices::{self, ActiveModel, Entity, Model};
use async_trait::async_trait;
use chrono::offset::Local;
use loco_rs::{auth::jwt, hash, prelude::*};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::controllers::v2::PilotAuthQuery;

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

impl super::_entities::devices::Model {

    pub async fn register_device(
        db: &DatabaseConnection,
        params: &PilotAuthQuery,
    ) -> Result<()> {
        Ok(())
    }
    /// Find all devices associated with a user
    /// 
    /// 
    /// Returns a list of devices associated with the user.
    /// Can be empty if the user has no devices
    pub async fn find_user_devices(
        db: &DatabaseConnection,
        user_id: i32,
    ) -> Vec<devices::Model> {
        devices::Entity::find()
            .filter(devices::Column::OwnerId.eq(user_id))
            .all(db)
            .await
            .expect("Database query failed")
    }

    pub async fn find_all_devices(
        db: &DatabaseConnection,
    ) -> Vec<Model> {
        Entity::find()
            .all(db)
            .await
            .expect("Database query failed")
    }

    pub async fn find_device(
        db: &DatabaseConnection,
        dongle_id: &String,
    ) -> Option<Model> {
        let result = Entity::find()
        .filter(devices::Column::DongleId.eq(dongle_id))
        .one(db)
        .await;
        match result {
            Ok(device) => device,
            Err(e) => {
                tracing::error!("DataBase Error: {:?}", e);
                None
            }
        }
    }
    

    pub async fn add_own_device(
        &self, 
        db: &DatabaseConnection, 
        params: &DeviceRegistrationParams) -> ModelResult<Model> {
        let txn = db.begin().await?;

        let device = devices::ActiveModel {
            dongle_id: ActiveValue::Set(params.dongle_id.to_string()),   
            //owner_id: ActiveValue::Set(self.owner_id),
            ..Default::default()
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;

        Ok(device)
    }

}