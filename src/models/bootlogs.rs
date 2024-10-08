use chrono::prelude::Utc;
use loco_rs::model::ModelResult;
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
pub use super::_entities::bootlogs::{self, ActiveModel, Model as BM, Entity, Column};

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

/// Implementation of the `Model` struct for routes.
impl BM {
    pub async fn add_bootlog(
        db: &DatabaseConnection, 
        dongle_id: &String, 
        bootlog_download_url: &String, 
        unlog_url: &String, 
        date_time: &String
    )  -> ModelResult<Self> {
        let txn = match db.begin().await {
            Ok(txn) => {
                tracing::debug!("Transaction began");
                txn
            }
            Err(e) => {
                tracing::error!("Failed to begin the transaction: {:?}", e);
                return Err(e.into());
            }
        };
        let bootlog = ActiveModel {
            dongle_id: ActiveValue::Set(dongle_id.clone()),
            bootlog_url: ActiveValue::Set(bootlog_download_url.clone()),
            unlog_url: ActiveValue::Set(unlog_url.clone()),
            date_time: ActiveValue::Set(date_time.clone()),
            ..Default::default()
        }
        .insert(&txn)
        .await;
        match bootlog {
            Ok(bootlog) => {
                tracing::debug!("Bootlog Added to database");
                txn.commit().await?;
                return Ok(bootlog);
            }
            Err(e) => {
                tracing::error!("Failed to add bootlog to database: {:?}", e);
                return Err(e.into());
            }
        };

    }

    pub async fn find_device_bootlogs(
        db: &DatabaseConnection,
        dongle_id: &String,
    ) -> ModelResult<Vec<BM>> {
        let routes = Entity::find()
            .filter(Column::DongleId.eq(dongle_id))
            .all(db)
            .await?;
        Ok(routes)
    }

}