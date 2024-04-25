use sea_orm::entity::prelude::*;
use super::_entities::routes::ActiveModel;

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

pub struct RouteParams {
    pub id: i32,
    pub cononical_route_name: String,
    pub url: String,
    pub user_id: i32,
}

impl super::_entities::users::Model {
    // pub async fn add_route(&self, db: &DatabaseConnection, route_id: i32) -> ModelResult<ActiveModel> {
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