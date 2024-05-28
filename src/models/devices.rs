use sea_orm::entity::prelude::*;
pub use super::_entities::devices::{self, ActiveModel, Entity, Model};
use loco_rs::{prelude::*};

use crate::controllers::v2::DeviceRegistrationParams;

// #[derive(Debug, Deserialize, Serialize)]
// pub struct DeviceRegistrationParams {
//     pub id: i32,
//     pub dongle_id: String,
//     pub serial_number: String,
//     pub user_id: i32,
//     pub sim_id: String,
// }

use chrono::prelude::{Utc,DateTime};
#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
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

impl super::_entities::devices::Model {

    pub async fn register_device(
        db: &DatabaseConnection,
        params: DeviceRegistrationParams,
        dongle_id: &String,
    ) -> ModelResult<()> {
        // Check if the device is registered already
        match devices::Model::find_device(db, dongle_id).await {
            Ok(_) => Ok(()),
            Err(e) => {
                // Add device to db
                let txn = db.begin().await?;
                let device = devices::Model {
                    dongle_id: dongle_id.clone(),
                    public_key: params.public_key,
                    imei: params.imei,
                    imei2: params.imei2,
                    serial: params.serial,
                    uploads_allowed: true,
                    ..Default::default()
                };
                device.into_active_model().insert(&txn).await?;
                txn.commit().await?;
                Ok(())
            },
        }
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
    ) -> ModelResult<Model> {
        let device = Entity::find()
            .filter(devices::Column::DongleId.eq(dongle_id))
            .one(db)
            .await?;
        device.ok_or_else(|| ModelError::EntityNotFound)
    }    

    // pub async fn add_own_device(
    //     &self, 
    //     db: &DatabaseConnection, 
    //     params: &DeviceRegistrationParams) -> ModelResult<Model> {
    //     let txn = db.begin().await?;

    //     let device = devices::ActiveModel {
    //         dongle_id: ActiveValue::Set(params.dongle_id.to_string()),   
    //         //owner_id: ActiveValue::Set(self.owner_id),
    //         ..Default::default()
    //     }
    //     .insert(&txn)
    //     .await?;

    //     txn.commit().await?;

    //     Ok(device)
    // }

}