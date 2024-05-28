use loco_rs::model::{ModelError, ModelResult};
use loco_rs::prelude::*;
use sea_orm::DeleteResult;
use sea_orm::{ActiveValue, TransactionTrait};
use super::_entities::routes::{self, ActiveModel, Entity, Model};
use migration::m20240424_000003_routes::Routes as RouteFields;



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

/// Implementation of the `Model` struct for routes.
impl super::_entities::routes::Model {
    /// Adds a route to the database.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the `DatabaseConnection`.
    /// * `rp` - A reference to the `RouteParams` struct containing the route parameters.
    ///
    /// # Returns
    ///
    /// Returns a `ModelResult` containing the added route on success, or an error on failure.
    pub async fn add_route_self(
        self,
        db: &DatabaseConnection,
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;
        if routes::Entity::find()
            .filter(routes::Column::Fullname.eq(&self.fullname))
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ModelError::EntityAlreadyExists {});
        }
        let active_model = self.clone().into_active_model();
        active_model.insert(&txn)
        .await?;
        txn.commit().await?;
        Ok(self)
        
    }
    /// Finds a route by its canonical route name.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the `DatabaseConnection`.
    /// * `canonical_route_name` - A reference to the canonical route name of the route.
    ///
    /// # Returns
    ///
    /// Returns a `ModelResult` containing the found route on success, or an error on failure.
    pub async fn find_route(
        db: &DatabaseConnection,
        fullname: &String,
    ) -> ModelResult<Model> {
        let route = Entity::find()
            .filter(routes::Column::Fullname.eq(fullname))
            .one(db)
            .await;
        match route {
            Ok(route) => match route {
                Some(route) => Ok(route),
                None => Err(ModelError::EntityNotFound),
            },
            Err(e) => {
                tracing::error!("DataBase Error: {:?}", e);
                Err(e.into())
            }
        }
    }

    // pub async fn find_all_routes(
    //     db: &DatabaseConnection,
    // ) -> ModelResult<Vec<Model>> {
    //     let routes = routes::Entity::find()
    //         .all(db)
    //         .await?;
    //     Ok(routes)
    // }
        //route.ok_or_else(|| ModelError::EntityNotFound)

    /// Finds all routes associated with a device.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the `DatabaseConnection`.
    /// * `device_dongle_id` - A reference to the device dongle ID.
    ///
    /// # Returns
    ///
    /// Returns a `ModelResult` containing a vector of found routes on success, or an error on failure.
    pub async fn find_device_routes(
        db: &DatabaseConnection,
        device_dongle_id: &String,
    ) -> ModelResult<Vec<Model>> {
        let routes = routes::Entity::find()
            .filter(routes::Column::DeviceDongleId.eq(device_dongle_id))
            .all(db)
            .await?;
        Ok(routes)
    }

    /// Deletes a route by its canonical route name.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the `DatabaseConnection`.
    /// * `canonical_route_name` - A reference to the canonical route name of the route to delete.
    ///
    /// # Returns
    ///
    /// Returns a `ModelResult` containing the delete result on success, or an error on failure.
    pub async fn delete_route(
        db: &DatabaseConnection,
        canonical_route_name: &String,
    ) -> ModelResult<DeleteResult> {
        Ok(routes::Entity::delete_by_id(canonical_route_name).exec(db).await?)
    }

    /// Deletes all routes associated with a device.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the `DatabaseConnection`.
    /// * `device_dongle_id` - A reference to the device dongle ID.
    ///
    /// # Returns
    ///
    /// Returns a `ModelResult` containing the delete result on success, or an error on failure.
    pub async fn delete_device_routes(
        db: &DatabaseConnection,
        device_dongle_id: &String,
    ) -> ModelResult<DeleteResult> {
        Ok(routes::Entity::delete_many()
            .filter(routes::Column::DeviceDongleId.eq(device_dongle_id))
            .exec(db)
            .await?)
    }
}

impl super::_entities::routes::ActiveModel {

}