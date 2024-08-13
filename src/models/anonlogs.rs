use sea_orm::entity::prelude::*;
use super::_entities::anonlogs::{ActiveModel, Model};
use loco_rs::{prelude::*};

use chrono::prelude::{Utc};
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

impl super::_entities::anonlogs::Model {

    pub async fn add_anonlog(
        db: &DatabaseConnection,
        url: &str,
    ) -> ModelResult<Model> {
        let txn: sea_orm::DatabaseTransaction = db.begin().await?;
        let log: Model = ActiveModel{
            url: ActiveValue::Set(url.into()),
            ..Default::default()
        }
        .insert(&txn).await?;
        txn.commit().await?;
        Ok(log)
    }
}