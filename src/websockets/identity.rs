use serde::{de::Error, Deserialize};
use serde_json::Error as SerdeError;
use base64;
use crate::models::devices;
use loco_rs::app::AppContext;
use jsonwebtoken::{
    decode, Algorithm, DecodingKey, Validation,
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

pub(crate) async fn verify(ctx: &AppContext, identity: &String, jwt: &str) -> bool {
    let device = match devices::Model::find_device(&ctx.db, identity).await {
        Some(device) => device,
        None => return false,
    };

    let claims = decode::<JWTPayload>(
        jwt,
        &DecodingKey::from_rsa_pem(&device.public_key.as_bytes()).unwrap(),
        &Validation::new(Algorithm::RS256),
    );
    match claims {
        Ok(_) => true,
        Err(_) => false,
    }

}

pub(crate) async fn verify_identity(ctx: &AppContext, jwt: &str) -> Result<JWTPayload, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.insecure_disable_signature_validation();
    let token_data = match decode::<JWTPayload>(
        jwt,
        &DecodingKey::from_secret(&"na".as_bytes()),
        &validation,
    ) {
        Ok(token_data) => token_data,
        Err(e) => return Err(e),
    };
    
    let device = match devices::Model::find_device(&ctx.db, &token_data.claims.identity).await {
        Some(device) => device,
        None => return Ok(token_data.claims),
    };

    let claims = decode::<JWTPayload>(
        jwt,
        &DecodingKey::from_rsa_pem(&device.public_key.as_bytes()).unwrap(),
        &Validation::new(token_data.header.alg),
    );
    match claims {
        Ok(token_data) => Ok(token_data.claims),
        Err(e) => Err(e),
    }
}