use loco_rs::model::ModelResult;
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use super::_entities::bootlogs::{self, ActiveModel, Entity, Model};

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

/// Implementation of the `Model` struct for routes.
impl super::_entities::bootlogs::Model {
    pub async fn add_bootlog(db: &DatabaseConnection, dongle_id: String, url: String, date_time: DateTime)  -> ModelResult<Self> {
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
        let bootlog = bootlogs::ActiveModel {
            dongle_id: ActiveValue::Set(dongle_id),
            url: ActiveValue::Set(url),
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
}