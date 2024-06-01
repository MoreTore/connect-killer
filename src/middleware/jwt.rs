//! # JSON Web Token (JWT) and Password Hashing
//!
//! This module provides functionality for working with JSON Web Tokens (JWTs)
//! and password hashing.

use jsonwebtoken::{
    decode, encode, errors::Result as JWTResult, get_current_timestamp, Algorithm, DecodingKey,
    EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};

/// Represents the default JWT algorithm used by the [`JWT`] struct.
const JWT_ALGORITHM: Algorithm = Algorithm::HS512;

/// Represents the claims associated with a user JWT.
#[derive(Debug, Serialize, Deserialize)]
pub struct UserClaims {
    pub identity: String,
    nbf: usize,
    iat: usize,
    exp: usize,
}

/// Represents the JWT configuration and operations.
///
/// # Example
/// ```rust
/// use loco_rs::auth;
///
/// auth::jwt::JWT::new("PqRwLF2rhHe8J22oBeHy");
/// ```
pub struct JWT {
    secret: String,
    algorithm: Algorithm,
}

impl JWT {
    /// Creates a new [`JWT`] instance with the specified secret key.
    #[must_use]
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_string(),
            algorithm: JWT_ALGORITHM,
        }
    }

    /// Override the default  JWT algorithm to be used.
    #[must_use]
    pub fn algorithm(mut self, algorithm: Algorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Generates a new JWT with specified claims and an expiration time.
    ///
    /// # Errors
    ///
    /// returns [`JWTResult`] error when could not generate JWT token. can be an
    /// invalid secret.
    ///
    /// # Example
    /// ```rust
    /// use loco_rs::auth;
    ///
    /// auth::jwt::JWT::new("PqRwLF2rhHe8J22oBeHy").generate_token(&604800, "PID".to_string());
    /// ```
    pub fn generate_token(&self, expiration: &u64, identity: String) -> JWTResult<String> {
        #[allow(clippy::cast_possible_truncation)]
        let exp = (get_current_timestamp() + expiration) as usize;
        let nbf = get_current_timestamp() as usize;
        let iat = nbf.clone();
        let claims = UserClaims { identity, exp, nbf, iat };

        let token = encode(
            &Header::new(self.algorithm),
            &claims,
            &EncodingKey::from_base64_secret(&self.secret)?,
        )?;

        Ok(token)
    }

    /// Validates the authenticity and expiration of a given JWT.
    /// If Token is valid, decode the Token Claims.
    ///
    /// # Errors
    ///
    /// returns [`JWTResult`] error when could not convert the given token to
    /// [`UserClaims`], if the `secret` is invalid or token is expired.
    ///
    /// # Example
    /// ```rust
    /// use loco_rs::auth;
    /// auth::jwt::JWT::new("PqRwLF2rhHe8J22oBeHy").validate("JWT-TOKEN");
    /// ```
    pub fn validate(&self, token: &str) -> JWTResult<TokenData<UserClaims>> {
        decode::<UserClaims>(
            token,
            &DecodingKey::from_base64_secret(&self.secret)?,
            &Validation::new(self.algorithm),
        )
    }

    pub fn validate_pem(&self, token: &str, pem: &[u8]) -> JWTResult<TokenData<UserClaims>> {
        decode::<UserClaims>(
            token,
            &DecodingKey::from_rsa_pem(pem)?,
            &Validation::new(self.algorithm),
        )
    }

    pub fn parse_unverified(&self, token: &str) -> JWTResult<TokenData<UserClaims>> {
        let mut insecure_validation = Validation::new(self.algorithm);
        insecure_validation.insecure_disable_signature_validation();
        insecure_validation.leeway = 30;
        insecure_validation.validate_nbf = true;
        decode::<UserClaims>(
            token,
            &DecodingKey::from_secret(&"no_secrete".as_bytes()),
            &insecure_validation
        )
    }
}

#[cfg(test)]
mod tests {

    use insta::{assert_debug_snapshot, with_settings};
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("valid token", 60)]
    #[case("token expired", 1)]
    #[tokio::test]
    async fn can_generate_token(#[case] test_name: &str, #[case] expiration: u64) {
        let jwt = JWT::new("PqRwLF2rhHe8J22oBeHy");
        let token = jwt.generate_token(&expiration, "pid".to_string()).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(3));
        with_settings!({filters => vec![
            (r"exp: (\d+),", "exp: EXP,")
        ]}, {
            assert_debug_snapshot!(test_name, jwt.validate(&token));
        });
    }
}
