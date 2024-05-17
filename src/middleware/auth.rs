
use async_trait::async_trait;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, HeaderMap}, response::{IntoResponse, Redirect},
};
use cookie::Cookie;
use serde::{Deserialize, Serialize};
use futures_util::TryFutureExt;
use loco_rs::{app::AppContext, auth, errors::Error};
use thiserror::Error;
// Define constants for token prefix and authorization header
const TOKEN_PREFIX: &str = "Bearer ";
const AUTH_HEADER: &str = "Authorization";
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
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("redirect to login")]
    RedirectToLogin,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AuthError::Unauthorized => (http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
            AuthError::RedirectToLogin => Redirect::to("/login").into_response(),
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
        let token = extract_jwt_from_cookie(&parts.headers).map_err(|_| AuthError::RedirectToLogin)?;

        let state: AppContext = AppContext::from_ref(state);

        let jwt_secret = state.config.get_jwt_config().map_err(|_| AuthError::RedirectToLogin)?;

        match auth::jwt::JWT::new(&jwt_secret.secret).validate(&token) {
            Ok(claims) => Ok(Self {
                claims: claims.claims,
            }),
            Err(_) => {
                tracing::trace!("Authentication Error. Redirecting to login");
                return Err(AuthError::RedirectToLogin);
            }
        }
    }
}

/// Function to extract a token from the cookies
///
/// # Errors
///
/// When token is not valid or not found
pub(crate) fn extract_jwt_from_cookie(headers: &HeaderMap) -> eyre::Result<String> {
    // Check if the 'cookie' header is present in the request
    if let Some(cookie_header) = headers.get("cookie") {
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

    Err(eyre::eyre!("JWT cookie not found"))
}

