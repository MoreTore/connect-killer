use sea_orm::entity::prelude::*;
pub use super::_entities::device_msg_queues::{self, ActiveModel, Entity, Model, Column};
use sea_orm::{ActiveValue, TransactionTrait, QueryOrder, QuerySelect};
use crate::controllers::ws::JsonRpcRequest;
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


impl super::_entities::device_msg_queues::Model {

    pub async fn get_latest_msg(
        db: &DatabaseConnection,
        dongle_id: &str,
    ) -> Result<Option<JsonRpcRequest>, DbErr> {
        if let Some(json_rpc_value) = Entity::find()
            .filter(Column::DongleId.eq(dongle_id))
            .order_by_desc(Column::CreatedAt)
            .select_only()
            .column(Column::JsonRpcRequest)
            .into_model::<serde_json::Value>()
            .one(db)
            .await?
        {
            match serde_json::from_value::<JsonRpcRequest>(json_rpc_value) {
                Ok(json_rpc_request) => Ok(Some(json_rpc_request)),
                Err(_) => Ok(None), // Return None if deserialization fails
            }
        } else {
            Ok(None)
        }
    }

    pub async fn find_latest_msg(
        db: &DatabaseConnection,
        dongle_id: &str,
    ) -> Result<Option<Model>, DbErr> {
        let msg = Entity::find()
            .filter(Column::DongleId.eq(dongle_id))
            .order_by_desc(Column::CreatedAt)
            .one(db)
            .await?;
    
        Ok(msg)
    }

    pub async fn insert_msg(
        db: &DatabaseConnection,
        dongle_id: &String,
        json_rpc_request: JsonRpcRequest,
    ) -> Result<(), DbErr> {
        // Create a new active model to insert into the database
        let new_msg = ActiveModel {
            dongle_id: ActiveValue::Set(dongle_id.clone()),
            json_rpc_request: ActiveValue::Set(serde_json::to_value(json_rpc_request).unwrap_or_default()),
            ..Default::default()
        };

        // Insert the new message into the database
        Entity::insert(new_msg).exec(db).await?;

        Ok(())
    }

    pub async fn delete_one_msg(
        db: &DatabaseConnection,
        id: &str,
    ) -> Result<(), DbErr> {
        // Find the latest message ID for the given dongle_id
        if let Some(latest_msg) = Entity::find()
            .filter(Column::Id.eq(id))
            .one(db)
            .await?
        {
            // Delete the message using the found ID
            Entity::delete_by_id(latest_msg.id).exec(db).await?;
        }

        Ok(())
    }

    pub async fn delete_all_msgs(
        db: &DatabaseConnection,
        dongle_id: &str,
    ) -> Result<(), DbErr> {
        // Delete all messages for the given dongle_id
        Entity::delete_many()
            .filter(Column::DongleId.eq(dongle_id))
            .exec(db)
            .await?;

        Ok(())
    }


}
