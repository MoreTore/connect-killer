use serde::{de::Error, Deserialize};
use serde_json::Error as SerdeError;
use base64;
use crate::models::devices;
use loco_rs::app::AppContext;
use jsonwebtoken::{
    decode, encode, errors::Result as JWTResult, get_current_timestamp, Algorithm, DecodingKey,
    EncodingKey, Header, TokenData, Validation,
};

#[derive(Debug, Deserialize)]
pub struct JWTPayload {
    pub identity: String,
    pub nbf: usize,
    pub iat: usize,
    pub exp: usize,
}


pub(crate) async fn extract_jwt_from_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    // Check if the 'cookie' header is present in the request
    if let Some(cookie_header) = headers.get("cookie") {
        // Convert the cookie header to a string
        for cookie in cookie::Cookie::split_parse_encoded(cookie_header.to_str().unwrap_or_default()) {
            let cookie = cookie.unwrap();
            match cookie.name() {
                "jwt" => {return Some(cookie.value().into());}
                _ => continue,
            }
        }
    }
    None
}

pub(crate) fn decode_jwt_identity(jwt: &str) -> Result<JWTPayload, SerdeError> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(SerdeError::custom("Invalid JWT: Does not contain 3 parts"));
    }

    let payload = parts[1];
    let payload_decoded_bytes: Vec<u8> = base64::Engine::decode(&base64::engine::GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        base64::engine::general_purpose::NO_PAD
    ), payload)
      .map_err(|e| SerdeError::custom(format!("Base64 decode error: {}", e)))?;

    //println!("Decoded payload bytes: {:?}", payload_decoded_bytes);

    let payload_str: Option<&str> = <dyn combine::parser::combinator::StrLike>::from_utf8(&payload_decoded_bytes);
    match payload_str {
        Some(payload) => serde_json::from_str(payload),
        None => return Err(SerdeError::custom("Invalid payload")),
    }
}

pub(crate) async fn verify(ctx: &AppContext, identity: String, jwt: &str) -> bool {
    let device = device::Model::find_device(ctx.db, identity).await;
    let claims = decode::<JWTPayload>(
        jwt,
        &DecodingKey::from_rsa_pem(&device.public_key.as_bytes()).unwrap(),
        &Validation::new(Algorithm::RS256),
    );
    match claims {
        Ok(claims) => Some(claims),
        Err(e) => None,
    }

}