#![allow(clippy::unused_async)]
use axum::extract::Query;
use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{
    decode, Algorithm, DecodingKey, TokenData, Validation,
};
use sha2::{Sha256, Digest};
use hex;

use crate::models::_entities;

#[derive(Debug, Deserialize, Serialize)]
pub struct DeviceClaims {
    register: bool,
    exp: i64,
}

///Query parameters
//
/// imei: Device IMEI
/// 
/// imei2: Device IMEI, second slot
/// 
/// serial: Device Serial
/// 
/// public_key: 2048-bit RSA Public Key
/// 
/// register_token: JWT token signed by your private key containing payload: {"register": True}
#[derive(Debug, Deserialize, Serialize)]
pub struct DeviceRegistrationParams {
    pub imei: String,
    pub imei2: String,
    pub serial: String,
    pub public_key: String,
    pub register_token: String
}
impl DeviceRegistrationParams {
    pub fn generate_dongle_id(&self) -> String {
        let mut hasher = Sha256::new();

        // Concatenate the device info
        hasher.update(&self.imei);
        hasher.update(&self.imei2);
        hasher.update(&self.serial);
        hasher.update(&self.public_key);

        // Compute the hash
        let result = hasher.finalize();

        // Encode the hash in hexadecimal and trim to 16 characters
        let hex_encoded = hex::encode(result);
        hex_encoded[0..16].to_string() // Take only the first 16 characters
    }
}

/// Key	    Type    Description
/// 
///"dongle_id"	    (string)	Dongle ID
/// 
///"access_token"	(string)    JWT token (see Authentication)
#[derive(Debug, Deserialize, Serialize)]
struct PilotAuthResponse {
    dongle_id: String,
    access_token: String,
}

pub async fn echo(req_body: String) -> String {
    req_body
}

pub async fn hello(State(_ctx): State<AppContext>) -> Result<Response> {
    // do something with context (database, etc)
    format::text("hello")
}

async fn decode_register_token(params: &DeviceRegistrationParams) -> Option<TokenData<DeviceClaims>> {
    //let mut validate = Validation::new(Algorithm::RS256);
    //validate.leeway = 0;
    let claims = decode::<DeviceClaims>(
        &params.register_token,
        &DecodingKey::from_rsa_pem(&params.public_key.as_bytes()).unwrap(),
        &Validation::new(Algorithm::RS256),
    );
    match claims {
        Ok(claims) => Some(claims),
        Err(e) => None,
    }
}

pub async fn pilotauth(
    State(ctx): State<AppContext>,
    Query(params): Query<DeviceRegistrationParams>
) -> impl IntoResponse {
    let _token = decode_register_token(&params).await;
    let dongle_id = params.generate_dongle_id();
    // TODO Add blacklist or whitelist here. Maybe a db table
    let result = _entities::devices::Model::register_device(&ctx.db, params, &dongle_id).await;
    match result {
        Ok(_) => (StatusCode::OK, format::json(PilotAuthResponse { dongle_id: dongle_id, access_token: "".into()})),
        Err(result) => (StatusCode::INTERNAL_SERVER_ERROR, Err(loco_rs::Error::Model(result))),
    }
}

/// pair_token	JWT Token signed by your EON private key containing payload {"identity": <dongle-id>, "pair": true}
#[derive(Debug, Deserialize, Serialize)]
struct DevicePairParams {
    pair_token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct DevicePairResponse {
    first_pair: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct DevicePairClaims {
    identity: String,
    pair: bool,
}

async fn decode_pair_token(ctx: &AppContext, jwt: &str) -> Result<DevicePairClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::RS256); //alg should not matter here
    validation.insecure_disable_signature_validation();
    let token_data = match decode::<DevicePairClaims>(
        jwt,
        &DecodingKey::from_secret(&"na".as_bytes()),
        &validation,
    ) {
        Ok(token_data) => token_data,
        Err(e) => return Err(e),
    };
    
    let device = match _entities::devices::Model::find_device(&ctx.db, &token_data.claims.identity).await {
        Ok(device) => device,
        Err(e) => return Ok(token_data.claims),
    };

    let claims = decode::<DevicePairClaims>(
        jwt,
        &DecodingKey::from_rsa_pem(&device.public_key.as_bytes()).unwrap(),
        &Validation::new(token_data.header.alg),
    );
    match claims {
        Ok(token_data) => Ok(token_data.claims),
        Err(e) => Err(e),
    }
}

async fn pilotpair(
    auth: crate::middleware::auth::MyJWT,
    State(ctx): State<AppContext>,
    Query(params): Query<DevicePairParams>
) -> Result<Response> {
    let claims = match decode_pair_token(&ctx, &params.pair_token).await {
        Ok(claims) => claims,
        Err(e) => {
            tracing::error!("Got and invalid pair token: {}", e);
            return format::json("Got and invalid pair token");//(StatusCode::BAD_REQUEST, format::json("Bad Token"));
        }
    };

    if claims.pair {
        let user_model = _entities::users::Model::find_by_pid(&ctx.db, &auth.claims.pid).await?;
        let mut device_model =  _entities::devices::Model::find_device(&ctx.db, &claims.identity).await?;
        let first_pair = device_model.owner_id.is_none();
        if first_pair { // only pair if it wasn't already
            device_model.owner_id = Some(user_model.id);
            let txn = ctx.db.begin().await?;
            device_model.into_active_model().insert(&txn).await?;
            txn.commit().await?;
        }
        format::json(DevicePairResponse { first_pair: first_pair})
    } else {
        return format::json("If you want to pair, 'pair' should be true!");
    } 
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("v2")
        .add("/", get(hello))
        .add("/echo", post(echo))
        .add("/pilotauth", post(pilotauth))
        .add("/pilotpair", post(pilotpair))
}
