
use std::{env, collections::HashMap};

use async_trait::async_trait;
use axum::{
    extract::{FromRef, FromRequestParts, Query, ws::WebSocketUpgrade},
    http::{request::Parts, HeaderMap, header}, response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie;
//use eyre::Error;
//use eyre::ErrReport;
use serde::{Deserialize, Serialize};
use futures_util::TryFutureExt;
use loco_rs::{app::AppContext, errors::Error, config::JWT as JWTConfig, prelude::*};
use thiserror::Error;

use super::jwt;
use crate::models::{devices::DM, users::UM};
// Define constants for token prefix and authorization header
const QUERY_TOKEN_PREFIX: &str = "sig";
const TOKEN_PREFIX: &str = "JWT ";
const AUTH_HEADER: &str = "Authorization";
const AUTH_COOKIE_NAME: &str = "jwt";

// Define a struct to represent user authentication information serialized
// to/from JSON
#[derive(Debug, Deserialize, Serialize)]
pub struct MyJWT {
    pub claims: jwt::UserClaims,
    pub device_model: Option<DM>,
    pub user_model: Option<UM>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct UnverifiedJWT {
    pub claims: jwt::UserClaims,
    pub device_model: Option<DM>,
    pub user_model: Option<UM>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SuperUserJWT {
    pub claims: jwt::UserClaims,
    pub device_model: Option<DM>,
    pub user_model: Option<UM>,
}

use jsonwebtoken::{errors::ErrorKind, Algorithm};

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
    error_kind: Option<String>, // Include the ErrorKind as an optional field
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unauthorized")]
    Unauthorized(String),
    #[error("redirect to login")]
    RedirectToLogin,
    #[error("reset dongleId")]
    ResetDone,
    #[error("server error")]
    InternalError,
    #[error("Error verifying jwt")]
    JWTError(ErrorKind)
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AuthError::Unauthorized(msg) => (http::StatusCode::UNAUTHORIZED, msg).into_response(),
            AuthError::RedirectToLogin => Redirect::to("/login").into_response(),
            AuthError::ResetDone => (http::StatusCode::ACCEPTED, "Reset your DongleId").into_response(),
            //AuthError::FormatError => (http::StatusCode::BAD_REQUEST, "Unauthorized").into_response(),
            AuthError::InternalError => (http::StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
            AuthError::JWTError(error_kind) => {
                let error_message = ErrorMessage {
                    code: http::StatusCode::UNAUTHORIZED.as_u16(),
                    message: "JWT validation failed".to_string(),
                    error_kind: Some(format!("{:?}", error_kind)), // Convert ErrorKind to a string
                };
                (http::StatusCode::UNAUTHORIZED, Json(error_message)).into_response()
            }
        }
    }
}


#[async_trait]
impl<S> FromRequestParts<S> for MyJWT
where
    AppContext: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ctx: AppContext = AppContext::from_ref(state);

        let token = extract_token(parts)?;
            //.map_err(|e| handle_unauth(parts, e))?;

        let jwt_secret = ctx.config.get_jwt_config().map_err(|_| AuthError::InternalError)?;

        let mut jwt_processor = jwt::JWT::new(&jwt_secret.secret); // algo defaults to HS512 which we use for users
        // extract the token data to get the algo
        let token_data = jwt_processor.parse_unverified(&token)
            .map_err(|e| handle_jwt_err(parts, e.kind()))?;

        let alg = token_data.header.alg;
        jwt_processor = jwt_processor.algorithm(token_data.header.alg);
        let identity = &token_data.claims.identity; // this is the dongle_id if its a device or identity if user
        let error_message = format!("identity: {} not registered", identity);
        match alg {
            Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => { // device can use these algos             
                let device = DM::find_device(&ctx.db, identity)
                    .await
                    .map_err(|_| handle_unauth(parts, &error_message))?; // return here if not registered

                let valid_token_data = jwt_processor.validate_pem(&token, device.public_key.as_bytes())
                    .map_err(|e| handle_unauth(parts, &format!("Got invalid token: {}", e.to_string())))?;

                return Ok(Self { 
                    claims: valid_token_data.claims, 
                    device_model: Some(device), 
                    user_model: None 
                });
            }
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => { // the server issues these to devices and users
                let valid_token_data = jwt_processor.validate(&token)
                    .map_err(|e| handle_unauth(parts, &format!("Got invalid token: {}", e.to_string())))?;

                let user_model = UM::find_by_identity(&ctx.db, identity).await;
                let device_model = DM::find_device(&ctx.db, identity).await;
                
                if user_model.is_err() && device_model.is_err() {
                    return Err(handle_unauth(parts, &error_message));
                } else if user_model.is_ok() && device_model.is_ok() {
                    panic!(); // should never get here
                }

                return Ok(Self {
                    claims: valid_token_data.claims,
                    user_model: user_model.ok(),
                    device_model: device_model.ok(),
                });
            }
            _ => return Err(handle_unauth(parts, "Must use RS or HS jwt algorithm"))
        }
    }
}

fn handle_jwt_err(parts: &mut Parts, error_kind: &ErrorKind) -> AuthError {
    let host_header = parts
        .headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    let parts: Vec<&str> = host_header.split('.').collect();

    if parts[0] == "useradmin" {
        // Redirect for useradmin subdomain
        return AuthError::RedirectToLogin;
    } else {
        // Return JWTError with the ErrorKind
        return AuthError::JWTError(error_kind.clone());
    }
}

fn handle_unauth(parts: &mut Parts, msg: &str) -> AuthError {
    let host_header = parts
        .headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    let parts: Vec<&str> = host_header.split('.').collect();

    if parts[0] == "useradmin" {
        // Redirect for useradmin subdomain
        return AuthError::RedirectToLogin;
    } else {
        // Return JWTError with the ErrorKind
        return AuthError::Unauthorized(msg.to_string());
    }
}


#[derive(Debug, Deserialize)]
pub struct DeviceClaims {
    pub identity: String,
    pub nbf: usize,
    pub iat: usize,
    pub exp: usize,
}

fn extract_token(parts: &mut Parts) -> Result<String, AuthError> {
    // Attempt to extract the token from the query string
    extract_token_from_query(QUERY_TOKEN_PREFIX, parts)
        .or_else(|_| {
            // If extracting from the query string fails, attempt to extract from cookies
            extract_token_from_cookie(AUTH_COOKIE_NAME, parts)
        })
        .or_else(|_| {
            // If extracting from cookies fails, attempt to extract from the authorization header
            extract_token_from_header(&parts.headers)
                .map_err(|e| handle_unauth(parts, &e))
        })
}
/// Function to extract a token from the authorization header
///
/// # Errors
///
/// When token is not valid or out found
pub fn extract_token_from_header(headers: &HeaderMap) -> Result<String, String> {
    Ok(headers
        .get(AUTH_HEADER)
        .ok_or_else(|| format!("Header {AUTH_HEADER} token not found"))?
        .to_str()
        .map_err(|err| err.to_string())?
        .strip_prefix(TOKEN_PREFIX)
        .ok_or_else(|| format!("Error strip {AUTH_HEADER} value"))?
        .to_string())
}

/// Extract a token value from cookie
///
/// # Errors
/// when token value from cookie is not found
pub fn extract_token_from_cookie(name: &str, parts: &Parts) -> Result<String, String> {

    let jar: cookie::CookieJar = cookie::CookieJar::from_headers(&parts.headers);
    Ok(jar
        .get(name)
        .ok_or("Token is not found".to_string())?
        .to_string()
        .strip_prefix(&format!("{name}="))
        .ok_or_else(|| "Error strip value".to_string())?
        .to_string())
}
/// Extract a token value from query
///
/// # Errors
/// when token value from cookie is not found
pub fn extract_token_from_query(name: &str, parts: &Parts) -> Result<String, String> {
    // LogoResult
    let parameters: Query<HashMap<String, String>> =
        Query::try_from_uri(&parts.uri).map_err(|err| err.to_string())?;
    parameters
        .get(name)
        .cloned()
        .ok_or_else(|| format!("`{name}` query parameter not found"))
}
