#![allow(clippy::unused_async)]
use std::env;
use sha2::{Sha256, Digest};
use hex;
use axum::{
    extract::{Form, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse},
	body::Body,
};
use loco_rs::prelude::*;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{
    decode, Algorithm, DecodingKey, TokenData, Validation,
};

use crate::models::_entities;
use crate::models::users::OAuthUserParams;

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
        Err(_e) => None,
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
        Err(_e) => return Ok(token_data.claims),
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
    Form(params): Form<DevicePairParams>
) -> Result<Response> {
    let claims = match decode_pair_token(&ctx, &params.pair_token).await {
        Ok(claims) => claims,
        Err(e) => {
            tracing::error!("Got and invalid pair token: {}", e);
            return format::json("Got an invalid pair token");//(StatusCode::BAD_REQUEST, format::json("Bad Token"));
        }
    };

    if claims.pair {
        let user_model = _entities::users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
        let device_model =  _entities::devices::Model::find_device(&ctx.db, &claims.identity).await?;
        let first_pair = device_model.owner_id.is_none();
        
        if first_pair { // only pair if it wasn't already
            let mut active_device_model = device_model.into_active_model();
            active_device_model.owner_id = ActiveValue::Set(Some(user_model.id));
            //let txn = ctx.db.begin().await?;
            active_device_model.update(&ctx.db).await?;
            //txn.commit().await?;
        }
        format::json(DevicePairResponse { first_pair: first_pair})
    } else {
        return format::json("If you want to pair, 'pair' should be true!");
    } 
}

#[derive(Debug, Deserialize, Serialize)]
struct GithubAuthParams {
    code: String,
    state: Option<String>,
    provider: Option<String>,
}


async fn github_redirect_handler(
    State(_ctx): State<AppContext>,
    Query(params): Query<GithubAuthParams>,
) -> Result<impl IntoResponse, StatusCode> {
    if let Some(state) = params.state {
        // Split the state to get the service URL
        let parts: Vec<&str> = state.split(',').collect();
        if parts.len() != 2 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let service_url = parts[1];
        let redirect_url = format!("https://{}/v2/auth/?provider=h&code={}", service_url, params.code);

        let mut headers = HeaderMap::new();
        headers.insert("Location", HeaderValue::from_str(&redirect_url).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?);

        return Ok((StatusCode::FOUND, headers));
    }
    Err(StatusCode::BAD_REQUEST)
}



#[derive(Deserialize, Serialize)]
pub struct GithubUser {
    pub id: u64,
    pub email: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct GithubTokenResponse {
    access_token: String,
}

async fn get_auth( // use for useradmin
    State(ctx): State<AppContext>,
	Query(params): Query<GithubAuthParams>,
) -> Result<Response> {
	let token_url = "https://github.com/login/oauth/access_token";
    let client = reqwest::Client::new();
    let response = client
        .post(token_url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", env::var("GITHUB_CLIENT").expect("GITHUB_CLIENT must be set")),
            ("client_secret", env::var("GITHUB_SECRET").expect("GITHUB_SECRET must be set")),
            ("code", params.code),
        ])
        .send().await;

    let response = match response {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("Github token error: {} ", e);
            return format::json("Failed");
        }
    };

    let token_response: GithubTokenResponse = response.json().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR).unwrap();

    let user_response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token_response.access_token))
        .header("User-Agent", "Connect")
        .send()
        .await;

    let user_response = match user_response {
        Ok(user_response) => user_response,
        Err(e) => {
            tracing::error!("Failed to get github user response: {} ", e);
            return unauthorized("Failed to get github user response");
        }
    };

    let github_user: GithubUser = match user_response.json().await {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to parse github user response: {} ", e);
            return unauthorized("Failed to parse github user response");
        }
    };
        
    let user = _entities::users::Model::with_oauth(
        &ctx.db, 
        &OAuthUserParams {
            name: format!("github_{}", github_user.id),
            email: None,
    }).await?;
    
    let jwt_secret = ctx.config.get_jwt_config()?;

    let token = user
        .generate_jwt(&jwt_secret.secret, &jwt_secret.expiration)
        .or_else(|_| unauthorized("Failed to generate token!"))?;

    // Set cookie and redirect
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        format!("jwt={}; Path=/; HttpOnly; Secure; Domain=.springerelectronics.com;", token).parse().unwrap(),
    );

    // Construct the redirect response manually
    let response = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::SET_COOKIE, headers.get(header::SET_COOKIE).unwrap().to_str().unwrap())
        .header(header::LOCATION, "/")
        .body(Body::empty())
        .unwrap();

    Ok(response)
}


async fn post_auth( // used for portal
    State(ctx): State<AppContext>,
	Form(params): Form<GithubAuthParams>,
) -> Result<Response> {
	
    let token_url = "https://github.com/login/oauth/access_token";
    let client = reqwest::Client::new();
    let response = client
        .post(token_url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", env::var("GITHUB_CLIENT").expect("GITHUB_CLIENT must be set")),
            ("client_secret", env::var("GITHUB_SECRET").expect("GITHUB_SECRET must be set")),
            ("code", params.code),
        ])
        .send().await;

    let response = match response {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("Github token error: {} ", e);
            return format::json("Failed");
        }
    };

    let token_response: GithubTokenResponse = response.json().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR).unwrap();

    let user_response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token_response.access_token))
        .header("User-Agent", "Connect")
        .send()
        .await;

    let user_response = match user_response {
        Ok(user_response) => user_response,
        Err(e) => {
            tracing::error!("Failed to get github user response: {} ", e);
            return unauthorized("Failed to get github user response");
        }
    };

    let github_user: GithubUser = match user_response.json().await {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to parse github user response: {} ", e);
            return unauthorized("Failed to parse github user response");
        }
    };
        
    let user = _entities::users::Model::with_oauth(
        &ctx.db, 
        &OAuthUserParams {
            name: format!("github_{}", github_user.id),
            email: None,
    }).await?;
    
    let jwt_secret = ctx.config.get_jwt_config()?;

    let token = user
        .generate_jwt(&jwt_secret.secret, &jwt_secret.expiration)
        .or_else(|_| unauthorized("Failed to generate token!"))?;

    format::json(GithubTokenResponse { access_token: token} )
}


pub fn routes() -> Routes {
    Routes::new()
        .prefix("v2")
        .add("/pilotauth", post(pilotauth))
        .add("/pilotpair", post(pilotpair))
        .add("/auth", post(post_auth).get(get_auth))
        .add("/auth/h/redirect", get(github_redirect_handler))
}
