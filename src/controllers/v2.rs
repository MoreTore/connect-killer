#![allow(clippy::unused_async)]
use axum::extract::Query;
use loco_rs::prelude::*;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{
    decode, encode, errors::Result as JWTResult, get_current_timestamp, Algorithm, DecodingKey,
    EncodingKey, Header, TokenData, Validation,
};

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
pub struct PilotAuthQuery {
    pub imei: String,
    pub imei2: String,
    pub serial: String,
    pub public_key: String,
    pub register_token: String
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

async fn decode_register_token(params: PilotAuthQuery) -> Option<TokenData<DeviceClaims>> {
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
    Query(mut params): Query<PilotAuthQuery>
) -> Result<Response> {
    let _token = decode_register_token(params).await;
    
    let jwt_secret = ctx.config.get_jwt_config()?;
    
    format::text("hello")
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("v2")
        .add("/", get(hello))
        .add("/echo", post(echo))
        .add("/pilotauth", post(pilotauth))
}
