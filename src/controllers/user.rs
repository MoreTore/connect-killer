use loco_rs::prelude::*;

use crate::{models::_entities::users, views::user::CurrentResponse};

async fn current(auth: crate::middleware::auth::MyJWT, State(ctx): State<AppContext>) -> Result<Response> {
    let user = users::Model::find_by_identity(&ctx.db, &auth.claims.identity).await?;
    format::json(CurrentResponse::new(&user))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/user")
        .add("/current", get(current))
}
