

use loco_rs::{prelude::*};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::middleware::jwt;
pub use super::_entities::users::{self, ActiveModel, Entity, Model};

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
    // #[validate(custom = "validation::is_valid_email")]
    // pub email: String,
}

impl Validatable for super::_entities::users::ActiveModel {
    fn validator(&self) -> Box<dyn Validate> {
        Box::new(Validator {
            name: self.name.as_ref().to_owned(),
            //email: self.email.as_ref().to_owned(),
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
            //this.api_key = ActiveValue::Set(format!("lo-{}", Uuid::new_v4()));
            Ok(this)
        } else {
            this.updated_at = ActiveValue::Set(Utc::now().naive_utc());
            Ok(this)
        }
    }
}


// #[async_trait]
// impl Authenticable for super::_entities::users::Model {
//     // async fn find_by_api_key(db: &DatabaseConnection, api_key: &str) -> ModelResult<Self> {
//     //     let user = users::Entity::find()
//     //         .filter(users::Column::ApiKey.eq(api_key))
//     //         .one(db)
//     //         .await?;
//     //     user.ok_or_else(|| ModelError::EntityNotFound)
//     // }

//     // async fn find_by_claims_key(db: &DatabaseConnection, claims_key: &str) -> ModelResult<Self> {
//     //     Self::find_by_pid(db, claims_key).await
//     // }
// }

impl super::_entities::users::Model {
    /// finds a user by the provided email
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    // pub async fn find_by_email(db: &DatabaseConnection, email: &str) -> ModelResult<Self> {
    //     let user = users::Entity::find()
    //         .filter(users::Column::Email.eq(email))
    //         .one(db)
    //         .await?;
    //     user.ok_or_else(|| ModelError::EntityNotFound)
    // }

    /// finds a user by the provided verification token
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    // pub async fn find_by_verification_token(
    //     db: &DatabaseConnection,
    //     token: &str,
    // ) -> ModelResult<Self> {
    //     let user = users::Entity::find()
    //         .filter(users::Column::EmailVerificationToken.eq(token))
    //         .one(db)
    //         .await?;
    //     user.ok_or_else(|| ModelError::EntityNotFound)
    // }

    /// /// finds a user by the provided reset token
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    // pub async fn find_by_reset_token(db: &DatabaseConnection, token: &str) -> ModelResult<Self> {
    //     let user = users::Entity::find()
    //         .filter(users::Column::ResetToken.eq(token))
    //         .one(db)
    //         .await?;
    //     user.ok_or_else(|| ModelError::EntityNotFound)
    // }
    pub async fn find_all_users(
        db: &DatabaseConnection,
    ) -> Vec<Model> {
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

    /// finds a user by the provided api key
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    // pub async fn find_by_api_key(db: &DatabaseConnection, api_key: &str) -> ModelResult<Self> {
    //     let user = users::Entity::find()
    //         .filter(users::Column::ApiKey.eq(api_key))
    //         .one(db)
    //         .await?;
    //     user.ok_or_else(|| ModelError::EntityNotFound)
    // }

    /// Verifies whether the provided plain password matches the hashed password
    ///
    /// # Errors
    ///
    /// when could not verify password
    // #[must_use]
    // pub fn verify_password(&self, password: &str) -> bool {
    //     hash::verify_password(password, &self.password)
    // }

    /// Asynchronously creates a user with a password and saves it to the
    /// database.
    ///
    /// # Errors
    ///
    /// When could not save the user into the DB
    // pub async fn create_with_password(
    //     db: &DatabaseConnection,
    //     params: &RegisterParams,
    // ) -> ModelResult<Self> {
    //     let txn = db.begin().await?;

    //     if users::Entity::find()
    //         .filter(users::Column::Email.eq(&params.email))
    //         .one(&txn)
    //         .await?
    //         .is_some()
    //     {
    //         return Err(ModelError::EntityAlreadyExists {});
    //     }

    //     let password_hash =
    //         hash::hash_password(&params.password).map_err(|e| ModelError::Any(e.into()))?;
    //     let user = users::ActiveModel {
    //         email: ActiveValue::set(params.email.to_string()),
    //         password: ActiveValue::set(password_hash),
    //         name: ActiveValue::set(params.name.to_string()),
    //         ..Default::default()
    //     }
    //     .insert(&txn)
    //     .await?;

    //     txn.commit().await?;

    //     Ok(user)
    // }
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

impl super::_entities::users::ActiveModel {

}
