use chrono::prelude::Utc;
use loco_rs::model::{ModelError, ModelResult};
use loco_rs::prelude::*;
use sea_orm::{ActiveValue, TransactionTrait, QuerySelect, DeleteResult, QueryOrder};
pub use super::_entities::routes::{self, ActiveModel, Entity, Model as RM, Column};



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
impl RM {
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
        if Entity::find()
            .filter(Column::Fullname.eq(&self.fullname))
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
    ) -> ModelResult<RM> {
        let route = Entity::find()
            .filter(Column::Fullname.eq(fullname))
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
            .filter(Column::DeviceDongleId.eq(dongle_id))
            .filter(Column::Hpgps.eq(true))
            .order_by_desc(Column::EndTimeUtcMillis)
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
    ) -> ModelResult<Vec<RM>> {
        let routes = Entity::find()
            .filter(Column::DeviceDongleId.eq(dongle_id))
            .filter(Column::StartTimeUtcMillis.gte(from))
            .filter(Column::StartTimeUtcMillis.lte(to))
            .order_by_desc(Column::CreatedAt)
            .limit(limit)
            .all(db).await?;
        Ok(routes)
    }

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
    ) -> ModelResult<Vec<RM>> {
        let routes = Entity::find()
            .filter(Column::DeviceDongleId.eq(dongle_id))
            .order_by_desc(Column::CreatedAt)
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

        let routes: Vec<f32> = Entity::find()
            .filter(Column::DeviceDongleId.eq(dongle_id))
            .select_only()
            .column(Column::Length)
            .into_tuple::<f32>() // Use f32 to match the SQL type
            .all(db)
            .await?;
    
        let total_length: f32 = routes.iter().sum(); // Sum all the lengths
        let route_count = routes.len() as u32; // Count the number of routes
    
        Ok((total_length, route_count))
    }

    pub async fn total_length_count_and_time_filtered(
        db: &DatabaseConnection,
        dongle_id: &str,
        from: Option<i64>,
        to: Option<i64>,
    ) -> ModelResult<(f32, u32, i64)> {
        use sea_orm::prelude::*;
        use sea_orm::{QuerySelect, Condition};
        
        let mut condition = Condition::all()
            .add(Column::DeviceDongleId.eq(dongle_id))
            .add(Column::Length.gt(0.1))
            .add(Expr::col(Column::StartTimeUtcMillis).lt(Expr::col(Column::EndTimeUtcMillis)));
                
        if let Some(from) = from {
            condition = condition.add(Column::StartTimeUtcMillis.gte(from));
        }
        
        if let Some(to) = to {
            condition = condition.add(Column::StartTimeUtcMillis.lte(to));
        }
        
        let routes: Vec<(f32, i64, i64)> = Entity::find()
            .filter(condition)
            .select_only()
            .columns([Column::Length, Column::StartTimeUtcMillis, Column::EndTimeUtcMillis])
            .into_tuple::<(f32, i64, i64)>()  // Fetch length, start, and end times
            .all(db)
            .await?;
        
        let total_length: f32 = routes.iter().map(|(length, _, _)| length).sum(); // Sum all the lengths
        let total_route_time: i64 = routes.iter().map(|(_, start, end)| end - start).sum(); // Sum total time (end - start) for each route
        let route_count = routes.len() as u32; // Count the number of routes
        
        Ok((total_length, route_count, total_route_time))
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
        Ok(Entity::delete_by_id(canonical_route_name).exec(db).await?)
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
        Ok(Entity::delete_many()
            .filter(Column::DeviceDongleId.eq(device_dongle_id))
            .exec(db)
            .await?)
    }
}

impl ActiveModel {

}