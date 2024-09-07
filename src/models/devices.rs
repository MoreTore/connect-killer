use chrono::prelude::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, TransactionTrait, QueryOrder};
use loco_rs::prelude::*;
pub use super::_entities::devices::{self, ActiveModel, Entity, Model as DM, Column};
use crate::controllers::v2::DeviceRegistrationParams;


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

impl DM {

    pub async fn register_device(
        db: &DatabaseConnection,
        params: DeviceRegistrationParams,
        dongle_id: &String,
    ) -> ModelResult<()> {
        // Check if the device is registered already
        match DM::find_device(db, dongle_id).await {
            Ok(_) => Ok(()),
            Err(_e) => {
                // Add device to db
                let txn = db.begin().await?;
                let device = DM {
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
    ) -> Vec<DM> {
        Entity::find()
            .filter(Column::OwnerId.eq(user_id))
            .order_by_desc(Column::Online)
            .order_by_desc(Column::LastAthenaPing)
            .all(db)
            .await
            .expect("Database query failed")
    }

    pub async fn find_user_device(
        db: &DatabaseConnection,
        user_id: i32,
        dongle_id: &str
    ) -> Result<Option<DM>, DbErr> {
        Entity::find()
            .filter(Column::OwnerId.eq(user_id))
            .filter(Column::DongleId.eq(dongle_id))
            .one(db)
            .await
    }

    pub async fn ensure_user_device(
        db: &DatabaseConnection,
        user_id: i32,
        dongle_id: &str
    ) -> Result<DM, DbErr> {
        Entity::find()
            .filter(Column::OwnerId.eq(user_id))
            .filter(Column::DongleId.eq(dongle_id))
            .one(db)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("Device not found for that owner".to_string()))
    }

    pub async fn find_all_devices(
        db: &DatabaseConnection,
    ) -> Vec<DM> {
        Entity::find()
            .order_by_desc(Column::Online)
            .order_by_desc(Column::LastAthenaPing)
            .all(db)
            .await
            .expect("Database query failed")
    }

    pub async fn find_device(
        db: &DatabaseConnection,
        dongle_id: &str,
    ) -> ModelResult<DM> {
        let device = Entity::find()
            .filter(Column::DongleId.eq(dongle_id))
            .one(db)
            .await?;
        device.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn reset_online(
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        // Update all devices to set `Online` to `false`
        Entity::update_many()
            .col_expr(Column::Online, Expr::value(false))
            .exec(db)
            .await?;
            
        Ok(())
    }

    pub async fn get_locations(
        db: &DatabaseConnection,
        dongle_id: &str,
    ) -> ModelResult<Option<serde_json::Value>> {
        let device = Entity::find()
            .filter(Column::DongleId.eq(dongle_id))
            .one(db)
            .await?;
        let device = device.ok_or(ModelError::EntityNotFound)?;
        // Return the optional JSON data stored in the locations field
        Ok(device.locations)
    }

}