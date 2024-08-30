use loco_rs::model::{ModelError, ModelResult};
use loco_rs::prelude::*;
use sea_orm::{ActiveValue, TransactionTrait, QuerySelect, DeleteResult, QueryOrder};
use super::_entities::routes::{self, ActiveModel, Entity, Model};




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

    pub async fn find_latest_pos(
        db: &DatabaseConnection,
        dongle_id: &str,
    ) -> ModelResult<(f64, f64, i64)> {
        let route = Entity::find()
            .filter(routes::Column::DeviceDongleId.eq(dongle_id))
            .filter(routes::Column::Hpgps.eq(true))
            .order_by_desc(routes::Column::EndTimeUtcMillis)
            .one(db)
            .await?
            .unwrap_or_default();

        Ok((route.end_lat, route.end_lng, route.end_time_utc_millis))
    }



    pub async fn find_time_filtered_device_routes(
        db: &DatabaseConnection,
        dongle_id: &str,
        from: Option<i64>,
        to: Option<i64>,
        limit: Option<u64>,
    ) -> ModelResult<Vec<routes::Model>> {
        let routes = routes::Entity::find()
            .filter(routes::Column::DeviceDongleId.eq(dongle_id))
            .filter(routes::Column::StartTimeUtcMillis.gte(from))
            .filter(routes::Column::StartTimeUtcMillis.lte(to))
            .order_by_desc(routes::Column::CreatedAt)
            .limit(limit)
            .all(db).await?;
        Ok(routes)
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
        dongle_id: &String,
    ) -> ModelResult<Vec<Model>> {
        let routes = routes::Entity::find()
            .filter(routes::Column::DeviceDongleId.eq(dongle_id))
            .order_by_desc(routes::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(routes)
    }

    pub async fn total_length_and_count(
        db: &DatabaseConnection,
        dongle_id: &String,
    ) -> ModelResult<(f32, u32)> {
        use sea_orm::prelude::*;
        use sea_orm::QuerySelect;

        let routes: Vec<f32> = routes::Entity::find()
            .filter(routes::Column::DeviceDongleId.eq(dongle_id))
            .select_only()
            .column(routes::Column::Length)
            .into_tuple::<f32>() // Use f32 to match the SQL type
            .all(db)
            .await?;
    
        let total_length: f32 = routes.iter().sum(); // Sum all the lengths
        let route_count = routes.len() as u32; // Count the number of routes
    
        Ok((total_length, route_count))
    }

    pub async fn total_length_and_count_time_filtered(
        db: &DatabaseConnection,
        dongle_id: &str,
        from: Option<i64>,
        to: Option<i64>,
    ) -> ModelResult<(f32, u32)> {
        use sea_orm::prelude::*;
        use sea_orm::QuerySelect;
        use sea_orm::Condition;
    
        let mut condition = Condition::all()
            .add(routes::Column::DeviceDongleId.eq(dongle_id));
    
        if let Some(from) = from {
            condition = condition.add(routes::Column::StartTimeUtcMillis.gte(from));
        }
    
        if let Some(to) = to {
            condition = condition.add(routes::Column::StartTimeUtcMillis.lte(to));
        }
    
        let routes: Vec<f32> = routes::Entity::find()
            .filter(condition)
            .select_only()
            .column(routes::Column::Length)
            .into_tuple::<f32>() // Use f32 to match the SQL type
            .all(db)
            .await?;
    
        let total_length: f32 = routes.iter().sum(); // Sum all the lengths
        let route_count = routes.len() as u32; // Count the number of routes
    
        Ok((total_length, route_count))
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