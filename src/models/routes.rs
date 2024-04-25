use sea_orm::entity::prelude::*;
use super::_entities::routes::{self, ActiveModel};

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

pub struct RouteParams {
    pub canonical_route_name: String,
    pub device_dongle_id: String,
    pub version: Option<String>,
    pub git_remote: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
    pub devicetype: Option<String>,
    pub can: bool,
    pub url: String,
    pub user_id: i32,
}

impl super::_entities::users::Model {
    // pub async fn add_route(&self, db: &DatabaseConnection, rp: &RouteParams) -> ModelResult<ActiveModel> {
    //     let txn = db.begin().await?;

    //     let route = routes::ActiveModel {
    //         route_id: ActiveValue::Set(route_id),
    //         user_id: ActiveValue::Set(self.id),
    //         ..Default::default()
    //     }
    //     .insert(&txn)
    //     .await?;

    //     txn.commit().await?;

    //     Ok(route)
    // }
}