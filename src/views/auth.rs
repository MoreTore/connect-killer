use serde::{Deserialize, Serialize};


use loco_rs::prelude::*;


#[derive(Debug, Deserialize, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub pid: String,
    pub name: String,
    pub is_verified: bool,
}


#[derive(Serialize)]
pub(crate) struct LoginTemplate {
    pub api_host: String,
}

pub fn login(v: impl ViewRenderer, template: LoginTemplate) -> Result<impl IntoResponse> {
    // Render the view with the template
    format::render().view(&v, "useradmin/login.html", template)
}
