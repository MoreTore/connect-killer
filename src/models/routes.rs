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
    pub can: bool,
    pub url: String,
    pub user_id: i32,
    pub public: bool
}

impl super::_entities::routes::Model {
    pub async fn add_route(db: &DatabaseConnection, rp: &RouteParams) -> ModelResult<Self> {
        let txn = db.begin().await;
        match txn {
            Ok(_) => {tracing::debug!("Transaction began");
                ()
            }
            Err(e) => {
                tracing::error!("Failed to begin the transaction: {:?}", e);
                return Err(e.into());
            }
        }
        let txn = txn?;
        if routes::Entity::find()
            .filter(routes::Column::CanonicalRouteName.eq(&rp.canonical_route_name))
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ModelError::EntityAlreadyExists {});
        }

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
            ..Default::default()
        }
        .insert(&txn)
        .await;
        match route {
            Ok(_) => {tracing::debug!("Route Added to database"); 
                ()}
            Err(e) => {
                tracing::error!("Failed to add route to database: {:?}", e);
                return Err(e.into());
            }
        }
        let route = route?;
        txn.commit().await?;
        Ok(route)
    }
    
    pub async fn find_route(
        db: &DatabaseConnection,
        canonical_route_name: &String,
    ) -> ModelResult<Model> {
        let route = Entity::find()
            .filter(routes::Column::CanonicalRouteName.eq(canonical_route_name))
            .one(db)
            .await?;
        route.ok_or_else(|| ModelError::EntityNotFound)
    }
    
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

    pub async fn delete_route(
        db: &DatabaseConnection,
        canonical_route_name: &String,
    ) -> ModelResult<DeleteResult> {
        Ok(routes::Entity::delete_by_id(canonical_route_name).exec(db).await?)  
    }

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

        RouteFields::GitDirty => self.git_dirty = ActiveValue::Set(Some(parse_value!(value, bool))),
        RouteFields::Public => self.public = ActiveValue::Set(parse_value!(value, bool)),
        RouteFields::Can => self.can = ActiveValue::Set(parse_value!(value, bool)),

        RouteFields::Devicetype => self.devicetype = ActiveValue::Set(Some(parse_value!(value, i16))),
        
        RouteFields::Table => (),

    }
    Ok(self.update(db).await?)
    }

}