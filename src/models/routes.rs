use loco_rs::model::{ModelError, ModelResult};
use loco_rs::{prelude::*};
use sea_orm::DeleteResult;
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use super::_entities::routes::{self, ActiveModel, Entity, Model};
use migration::m20240424_000003_routes::Routes as RouteFields;



impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

#[derive(Default, Clone)]
pub struct RouteParams {
    pub canonical_route_name: String,
    pub device_dongle_id: String,
    pub version: Option<String>,
    pub git_remote: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
    pub devicetype: Option<i16>,
    pub max_qlog: i16,
    pub max_qcam: i16,
    pub platform: String,
    pub can: bool,
    pub url: String,
    pub user_id: i32,
    pub public: bool,
    pub start_time: i64,
    pub miles: f32,
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
    pub async fn add_route(db: &DatabaseConnection, rp: &RouteParams) -> ModelResult<Self> {
        // Begin a transaction
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

        // Check if the route already exists
        if routes::Entity::find()
            .filter(routes::Column::CanonicalRouteName.eq(&rp.canonical_route_name))
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ModelError::EntityAlreadyExists {});
        }

        // Create a new route
        let route = routes::ActiveModel {
            canonical_route_name: ActiveValue::Set(rp.canonical_route_name.clone()),
            device_dongle_id: ActiveValue::Set(rp.device_dongle_id.clone()),
            version: ActiveValue::Set(rp.version.clone()),
            git_remote: ActiveValue::Set(rp.git_remote.clone()),
            git_branch: ActiveValue::Set(rp.git_branch.clone()),
            git_commit: ActiveValue::Set(rp.git_commit.clone()),
            git_dirty: ActiveValue::Set(Some(rp.git_dirty)),
            devicetype: ActiveValue::Set(rp.devicetype),
            can: ActiveValue::Set(rp.can),
            url: ActiveValue::Set(rp.url.clone()),
            public: ActiveValue::Set(rp.public),
            start_time: ActiveValue::Set(rp.start_time),
            miles: ActiveValue::Set(rp.miles),
            ..Default::default()
        }
        .insert(&txn)
        .await;

        let route = match route {
            Ok(route) => {
                tracing::debug!("Route Added to database");
                route
            }
            Err(e) => {
                tracing::error!("Failed to add route to database: {:?}", e);
                return Err(e.into());
            }
        };

        // Commit the transaction
        txn.commit().await?;

        Ok(route)
    }

    pub async fn add_route_self(
        self,
        db: &DatabaseConnection,
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;
        if routes::Entity::find()
            .filter(routes::Column::CanonicalRouteName.eq(&self.canonical_route_name))
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
        canonical_route_name: &String,
    ) -> ModelResult<Model> {
        let route = Entity::find()
            .filter(routes::Column::CanonicalRouteName.eq(canonical_route_name))
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
    pub async fn update_field(
        mut self,
        db: &DatabaseConnection,
        field: &RouteFields,
        value: &String,
    ) -> ModelResult<Model> {
    macro_rules! parse_value {
        ($value:expr, $type:ty) => {
            $value.parse::<$type>().map_err(|e| {
                tracing::error!("Failed to parse value: '{}', error: {}", $value, e);
                ModelError::EntityNotFound
            })?
        };
    }
    match field {
        RouteFields::CanonicalRouteName => self.canonical_route_name = ActiveValue::Set(value.to_string()),
        RouteFields::DeviceDongleId => self.device_dongle_id = ActiveValue::Set(value.to_string()),
        RouteFields::GitCommit => self.git_commit = ActiveValue::Set(Some(value.to_string())),
        RouteFields::Url => self.url = ActiveValue::Set(value.to_string()),
        RouteFields::GitBranch => self.git_branch = ActiveValue::Set(Some(value.to_string())),
        RouteFields::Version => self.version = ActiveValue::Set(Some(value.to_string())),
        RouteFields::GitRemote => self.git_remote = ActiveValue::Set(Some(value.to_string())),
        RouteFields::Platform => self.platform = ActiveValue::Set(value.to_string()),

        RouteFields::GitDirty => self.git_dirty = ActiveValue::Set(Some(parse_value!(value, bool))),
        RouteFields::Public => self.public = ActiveValue::Set(parse_value!(value, bool)),
        RouteFields::Can => self.can = ActiveValue::Set(parse_value!(value, bool)),

        RouteFields::Devicetype => self.devicetype = ActiveValue::Set(Some(parse_value!(value, i16))),
        RouteFields::MaxQcam => self.max_qcam = ActiveValue::Set(parse_value!(value, i16)),
        RouteFields::MaxQlog => self.max_qlog = ActiveValue::Set(parse_value!(value, i16)),

        RouteFields::Miles => self.miles = ActiveValue::Set(parse_value!(value, f32)),

        RouteFields::StartTime => self.start_time = ActiveValue::Set(parse_value!(value, i64)),
        
        RouteFields::Table => (),

    }
    Ok(self.update(db).await?)
    }

}