#![allow(clippy::unused_async)]
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use sysinfo::{
    Disks
};
use crate::{
    models::{
        devices::DM,
        routes::RM,
    }
};
use crate::{
    views
};
#[derive(Serialize, Deserialize)]
pub struct CpuUsage {
    core: u8,
    usage: f32,
}
#[derive(Serialize, Deserialize)]
pub struct DiskSpace {
    total: u64,
    used: u64,
    free: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Active {
    daily: u64,
    weekly: u64,
    monthly: u64,
    quarterly: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Devices {
    online: u64,
    total: u64,
    active: Active,
}
#[derive(Serialize, Deserialize)]
pub struct Network {
    current_upload: f32,
    current_download: f32,
    total_upload: f32,
    total_download: f32,
}
#[derive(Serialize, Deserialize)]
pub struct DriveStats {
    total_miles: i32,
    total_drives: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ServerUsage {
    time: String,
    disk_usage: Vec<DiskSpace>,
    //users: u64,
    devices: Devices,
    drive_stats: DriveStats,
}


async fn get_disk_usage() -> Vec<DiskSpace> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|disk| disk.name() == "/dev/md0")
        .map(|disk| DiskSpace {
            total: disk.total_space(),
            used: disk.total_space() - disk.available_space(),
            free: disk.available_space(),
        })
        .collect()
}

pub async fn get_server_usage(
    ViewEngine(view): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse> {
    let utc_time_now_millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u64;
    let one_day_ago_millis = utc_time_now_millis - std::time::Duration::from_secs(24 * 60 * 60).as_secs() as u64;
    let one_week_ago_millis = utc_time_now_millis - std::time::Duration::from_secs(7 * 24 * 60 * 60).as_secs() as u64;
    let one_month_ago_millis = utc_time_now_millis - std::time::Duration::from_secs(30 * 24 * 60 * 60).as_secs() as u64;
    let three_months_ago_millis = utc_time_now_millis - std::time::Duration::from_secs(90 * 24 * 60 * 60).as_secs() as u64;
    
    
    views::route::server_usage(
        view,
        ServerUsage {
            time: chrono::Utc::now().to_rfc3339(),
            disk_usage:  get_disk_usage().await,
            devices: Devices {
                total: DM::get_registered_devices(&ctx.db,None, None, None).await?,
                online: DM::get_registered_devices(&ctx.db,Some(true), None, None).await?,
                active: Active {
                    daily: DM::get_registered_devices(&ctx.db,None, None, Some(one_day_ago_millis)).await?,
                    weekly: DM::get_registered_devices(&ctx.db,None, None, Some(one_week_ago_millis)).await?,
                    monthly: DM::get_registered_devices(&ctx.db,None, None, Some(one_month_ago_millis)).await?,
                    quarterly: DM::get_registered_devices(&ctx.db,None, None, Some(three_months_ago_millis)).await?

                },
            },
            drive_stats: DriveStats { 
                total_miles: RM::get_miles(&ctx.db).await? as i32, 
                total_drives: RM::get_drive_count(&ctx.db).await?
            }
        },
    )
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("stats")
        .add("/usage", get(get_server_usage))
}
