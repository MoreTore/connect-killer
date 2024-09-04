

use loco_rs::{prelude::*};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::middleware::jwt;
pub use super::_entities::users::{self, ActiveModel, Entity, Model as UM};

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterParams {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OAuthUserParams {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Validate, Deserialize)]
pub struct Validator {
    #[validate(length(min = 2, message = "Name must be at least 2 characters long."))]
    pub name: String,
}

impl Validatable for super::_entities::users::ActiveModel {
    fn validator(&self) -> Box<dyn Validate> {
        Box::new(Validator {
            name: self.name.as_ref().to_owned(),
        })
    }
}
use chrono::prelude::{Utc};
#[async_trait::async_trait]
impl ActiveModelBehavior for super::_entities::users::ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        self.validate()?;
        let mut this = self;
        if insert {
            
            this.identity = ActiveValue::Set(Uuid::new_v4());
            this.created_at = ActiveValue::Set(Utc::now().naive_utc());
            this.updated_at = ActiveValue::Set(Utc::now().naive_utc());
            Ok(this)
        } else {
            this.updated_at = ActiveValue::Set(Utc::now().naive_utc());
            Ok(this)
        }
    }
}



impl super::_entities::users::Model {
    pub async fn find_all_users(
        db: &DatabaseConnection,
    ) -> Vec<UM> {
        Entity::find()
            .all(db)
            .await
            .expect("Database query failed")
    }
    /// finds a user by the provided pid
    ///
    /// # Errors
    ///
    /// When could not find user  or DB query error
    pub async fn find_by_identity(db: &DatabaseConnection, identity: &str) -> ModelResult<Self> {
        let parse_uuid = Uuid::parse_str(identity).map_err(|e| ModelError::Any(e.into()))?;
        let user = users::Entity::find()
            .filter(users::Column::Identity.eq(parse_uuid))
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_by_name(db: &DatabaseConnection, name: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(users::Column::Name.eq(name))
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(users::Column::Id.eq(id))
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn with_oauth(
        db: &DatabaseConnection,
        params: &OAuthUserParams,
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;

        match users::Entity::find()
            .filter(users::Column::Name.eq(&params.name))
            .one(&txn)
            .await?
        {
            Some(user) => {
                txn.commit().await?;
                return Ok(user);
            }
            None => {
                let user = users::ActiveModel {
                    email: ActiveValue::Set(params.email.clone()),
                    name: ActiveValue::Set(params.name.clone()),
                    points: ActiveValue::Set(0),
                    superuser: ActiveValue::Set(false),
                    ..Default::default()
                }
                .insert(&txn)
                .await?;
        
                txn.commit().await?;
        
                Ok(user)
            } 
        }
    }



    /// Creates a JWT
    ///
    /// # Errors
    ///
    /// when could not convert user claims to jwt token
    pub fn generate_jwt(&self, secret: &str, expiration: &u64) -> ModelResult<String> {
        Ok(jwt::JWT::new(secret).generate_token(expiration, self.identity.to_string())?)
    }
}

impl ActiveModel {

}
