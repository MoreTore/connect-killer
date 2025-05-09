use loco_rs::prelude::*;

use crate::controllers::{connectdata::UlogText, useradmin::{CloudlogsTemplate, MasterTemplate}};

pub fn admin_route(v: impl ViewRenderer, template: MasterTemplate) -> Result<impl IntoResponse> {
    // Render the view with the template
    format::render().view(&v, "useradmin/template.html", template)
}

pub fn admin_cloudlogs(v: impl ViewRenderer, template: CloudlogsTemplate) -> Result<impl IntoResponse> {
    // Render the cloudlog.html view with an empty context
    format::render().view(&v, "useradmin/cloudlog.html", template)
}

pub fn admin_segment_ulog(v: impl ViewRenderer, data: UlogText) -> Result<impl IntoResponse> {
    // Render the view with the template
    format::render().view(&v, "useradmin/ulog.html", data)
}