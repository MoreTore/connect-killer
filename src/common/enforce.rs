use loco_rs::prelude::*;

#[macro_export]
macro_rules! enforce_ownership_rule {
    ($user_id:expr, $owner_id:expr, $msg:expr) => {{
        if let Some(owner_id) = $owner_id {
            if $user_id != owner_id {
                tracing::error!("Someone is trying to make illegal access: {}", $msg);
                return loco_rs::controller::unauthorized($msg);
            }
        } else {
            tracing::error!("Someone is trying to make illegal access: {}", $msg);
            return loco_rs::controller::unauthorized($msg);
        }
    }};
}

#[macro_export]
macro_rules! enforce_device_upload_permission {
    ($auth:expr) => {{
        if let Some(device_model) = &$auth.device_model {
            if !device_model.uploads_allowed {
                return loco_rs::controller::unauthorized("Uploads ignored");
            }
        } else {
            return loco_rs::controller::unauthorized("Only registered devices can upload");
        }
    }};
}