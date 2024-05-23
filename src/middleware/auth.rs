
use std::collections::HashMap;

use async_trait::async_trait;
use axum::{
    extract::{FromRef, FromRequestParts, Query},
    http::{request::Parts, HeaderMap}, response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie;
//use eyre::Error;
//use eyre::ErrReport;
use serde::{Deserialize, Serialize};
use futures_util::TryFutureExt;
use loco_rs::{app::AppContext, auth, errors::Error, config::JWT as JWTConfig};
use thiserror::Error;

use super::identity;
use crate::models::_entities::{devices, users};
// Define constants for token prefix and authorization header
const QUERY_TOKEN_PREFIX: &str = "sig";
const TOKEN_PREFIX: &str = "JWT ";
const AUTH_HEADER: &str = "Authorization";
const AUTH_COOKIE: &str = "cookie";
const AUTH_COOKIE_NAME: &str = "jwt";

// Define a struct to represent user authentication information serialized
// to/from JSON
// #[derive(Debug, Deserialize, Serialize)]
// pub struct JWTWithUser<T: Authenticable> {
//     pub claims: auth::jwt::UserClaims,
//     pub user: T,
// }

// Define a struct to represent user authentication information serialized
// to/from JSON
#[derive(Debug, Deserialize, Serialize)]
pub struct MyJWT {
    pub claims: auth::jwt::UserClaims,
    pub device_model: Option<devices::Model>,
    pub user_model: Option<users::Model>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SuperUserJWT {
    pub claims: auth::jwt::UserClaims,
    pub device_model: Option<devices::Model>,
    pub user_model: Option<users::Model>,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("redirect to login")]
    RedirectToLogin,
    #[error("jwt format error")]
    FormatError,
    #[error("server error")]
    InternalError,

}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AuthError::Unauthorized => (http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
            AuthError::RedirectToLogin => Redirect::to("/login").into_response(),
            AuthError::FormatError => (http::StatusCode::BAD_REQUEST, "Unauthorized").into_response(),
            AuthError::InternalError => (http::StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
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

        let token = extract_token(get_jwt_from_config(&ctx).map_err(|_| AuthError::InternalError)?, parts)
            .map_err(|_| AuthError::FormatError)?;

        let jwt_secret = ctx.config.get_jwt_config().map_err(|_| AuthError::InternalError)?;

        let mut jwt_processor = auth::jwt::JWT::new(&jwt_secret.secret);

        let token_data = match jwt_processor.parse_unverified(&token) {
            Ok(token_data) => token_data,
            Err(_) => return Err(AuthError::RedirectToLogin),
        };

        if let Ok(token_data) = jwt_processor.validate(&token) {
            // Try to find the user model by identity
            match users::Model::find_by_identity(&ctx.db, &token_data.claims.identity).await {
                Ok(user_model) => {
                    return Ok(Self {
                        claims: token_data.claims,
                        user_model: Some(user_model),
                        device_model: None,
                    });
                }
                Err(_) => {
                    // If user model is not found, try to find the device model
                    match devices::Model::find_device(&ctx.db, &token_data.claims.identity).await {
                        Ok(device_model) => {
                            return Ok(Self {
                                claims: token_data.claims,
                                user_model: None,
                                device_model: Some(device_model),
                            });
                        }
                        Err(_) => {
                            // If neither user model nor device model is found, return an unauthorized error
                            return Err(AuthError::Unauthorized);
                        }
                    }
                }
            }
        }

        jwt_processor = jwt_processor.algorithm(token_data.header.alg);
        let device = devices::Model::find_device(&ctx.db, &token_data.claims.identity).await
            .map_err(|_| AuthError::Unauthorized)?;

        if let Ok(token_data) = jwt_processor.validate_pem(&token, device.public_key.as_bytes()) {
            return Ok(Self { claims: token_data.claims, device_model: Some(device), user_model: None });
        }

        Err(AuthError::RedirectToLogin)
    }
}

#[derive(Debug, Deserialize)]
pub struct DeviceClaims {
    pub identity: String,
    pub nbf: usize,
    pub iat: usize,
    pub exp: usize,
}

/// Function to extract a token from the cookies
///
/// # Errors
///
/// When token is not valid or not found
pub(crate) fn extract_jwt_from_cookie(headers: &HeaderMap) -> eyre::Result<String> {
    // Check if the 'cookie' header is present in the request
    if let Some(cookie_header) = headers.get(AUTH_COOKIE) {
        // Convert the cookie header to a string
        let cookie_str = cookie_header.to_str().map_err(|e| eyre::eyre!("Invalid cookie header: {}", e))?;
        
        // Parse the cookie header
        for cookie in cookie::Cookie::split_parse_encoded(cookie_str) {
            let cookie = cookie.map_err(|e| eyre::eyre!("Failed to parse cookie: {}", e))?;
            if cookie.name() == AUTH_HEADER {
                return Ok(cookie.value().to_string());
            }
            // if cookie.name() == "jwt" {
            //     return Ok(cookie.value().to_string());
            // }
        }
    }
    if let Some(authorization_header) = headers.get(AUTH_HEADER) {
        let auth_str = authorization_header.to_str().map_err(|e| eyre::eyre!("Failed to parse authorization header: {}", e))?;

        match auth_str.to_string().strip_prefix("JWT ") {
            Some(token) =>  return Ok(token.to_string()),
            None => (),
        }

    }
    Err(eyre::eyre!("JWT cookie not found"))
    
}

/// extract JWT token from context configuration
///
/// # Errors
/// Return an error when JWT token not configured
fn get_jwt_from_config(ctx: &AppContext) -> Result<&JWTConfig, Error> {
    ctx.config
        .auth
        .as_ref()
        .ok_or_else(|| Error::string("auth not configured"))?
        .jwt
        .as_ref()
        .ok_or_else(|| Error::string("JWT token not configured"))
}
/// extract token from the configured jwt location settings
// fn extract_token(jwt_config: &JWTConfig, parts: &Parts) -> Result<String, Error> {
//     #[allow(clippy::match_wildcard_for_single_variants)]
//     match jwt_config
//         .location
//         .as_ref()
//         .unwrap_or(&loco_rs::config::JWTLocation::Bearer)
//     {
//         loco_rs::config::JWTLocation::Query { name } => extract_token_from_query(name, parts),
//         loco_rs::config::JWTLocation::Cookie { name } => extract_token_from_cookie(name, parts),
//         loco_rs::config::JWTLocation::Bearer => extract_token_from_header(&parts.headers)
//             .map_err(|e| Error::Unauthorized(e.to_string())),
//     }
// }

fn extract_token(jwt_config: &JWTConfig, parts: &Parts) -> Result<String, Error> {
    // Attempt to extract the token from the query string
    extract_token_from_query(QUERY_TOKEN_PREFIX, parts)
        .or_else(|_| {
            // If extracting from the query string fails, attempt to extract from cookies
            extract_token_from_cookie(AUTH_COOKIE_NAME, parts)
        })
        .or_else(|_| {
            // If extracting from cookies fails, attempt to extract from the authorization header
            extract_token_from_header(&parts.headers)
                .map_err(|e| Error::Unauthorized(e.to_string()))
        })
}
/// Function to extract a token from the authorization header
///
/// # Errors
///
/// When token is not valid or out found
pub fn extract_token_from_header(headers: &HeaderMap) -> Result<String, Error> {
    Ok(headers
        .get(AUTH_HEADER)
        .ok_or_else(|| Error::Unauthorized(format!("header {AUTH_HEADER} token not found")))?
        .to_str()
        .map_err(|err| Error::Unauthorized(err.to_string()))?
        .strip_prefix(TOKEN_PREFIX)
        .ok_or_else(|| Error::Unauthorized(format!("error strip {AUTH_HEADER} value")))?
        .to_string())
}

/// Extract a token value from cookie
///
/// # Errors
/// when token value from cookie is not found
pub fn extract_token_from_cookie(name: &str, parts: &Parts) -> Result<String, Error> {
    // LogoResult
    let jar: cookie::CookieJar = cookie::CookieJar::from_headers(&parts.headers);
    Ok(jar
        .get(name)
        .ok_or(Error::Unauthorized("token is not found".to_string()))?
        .to_string()
        .strip_prefix(&format!("{name}="))
        .ok_or_else(|| Error::Unauthorized("error strip value".to_string()))?
        .to_string())
}
/// Extract a token value from query
///
/// # Errors
/// when token value from cookie is not found
pub fn extract_token_from_query(name: &str, parts: &Parts) -> Result<String, Error> {
    // LogoResult
    let parameters: Query<HashMap<String, String>> =
        Query::try_from_uri(&parts.uri).map_err(|err| Error::Unauthorized(err.to_string()))?;
    parameters
        .get(name)
        .cloned()
        .ok_or_else(|| Error::Unauthorized(format!("`{name}` query parameter not found")))
}
