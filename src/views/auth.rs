use serde::{Deserialize, Serialize};

use crate::models::_entities::users;
use loco_rs::prelude::*;


#[derive(Debug, Deserialize, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub pid: String,
    pub name: String,
    pub is_verified: bool,
}

impl LoginResponse {
    #[must_use]
    pub fn new(user: &users::Model, token: &String) -> Self {
        Self {
            token: token.to_string(),
            pid: user.pid.to_string(),
            name: user.name.clone(),
            is_verified: user.email_verified_at.is_some(),
        }
    }
}

#[derive(Serialize)]
struct LoginTemplate {
    api_host: String,
}

pub fn login(v: impl ViewRenderer) -> Result<impl IntoResponse> {
    // Render the view with the template
    format::render().view(&v, "useradmin/login.html", LoginTemplate {api_host: "http://154.38.175.6:3111".into() })
}
