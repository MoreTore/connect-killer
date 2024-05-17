use loco_rs::prelude::*;

use crate::controllers::{connectdata::UlogText, useradmin::MasterTemplate};

pub fn admin_route(v: impl ViewRenderer, template: MasterTemplate) -> Result<impl IntoResponse> {
    // Render the view with the template
    format::render().view(&v, "useradmin/route/route.html", template)
}

pub fn admin_segment_ulog(v: impl ViewRenderer, data: UlogText) -> Result<impl IntoResponse> {
    // Render the view with the template
    format::render().view(&v, "useradmin/segment/ulog.html", data)
}