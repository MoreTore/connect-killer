use serde::{Deserialize, Serialize};

/// ## Device Info Response
/// GET /v1.1/devices/:dongle_id/
///
#[derive(Serialize, Debug, Default)]
pub struct DeviceInfoResponse {
    pub dongle_id: String,            /// see Dongle ID
    pub alias: String,                /// Globally unique device nickname
    pub serial: String,               /// Device serial
    pub athena_host: String,          /// Last connected athena server
    pub last_athena_ping: i64,        /// (integer)	Timestamp of last athena ping
    pub ignore_uploads: bool,         /// If uploads are ignored
    pub is_paired: bool,              /// Device has an owner
    pub is_owner: bool,               /// Authed user has write-access to device
    pub public_key: String,           /// 2048-bit public RSA key
    pub prime: bool,                  /// If device has prime
    pub prime_type: i16,              /// Prime type: 0: no prime, 1: standard prime, 2: prime lite
    pub trial_claimed: bool,          /// If device prime trial is claimed
    pub device_type: String,          /// one of ("neo", "panda", "app")
    pub last_gps_time: i64,           /// Milliseconds since epoch of last gps. Updates upon successful call to device location endpoint
    pub last_gps_lat: f64,            /// Latitude of last location
    pub last_gps_lng: f64,            /// Longitude of last location
    pub last_gps_accur: f64,          /// Accuracy (m) of last location
    pub last_gps_speed: f64,          /// Speed (m/s) at last location
    pub last_gps_bearing: f64,        /// Direction of last location in degrees from north
    pub openpilot_version: String,    /// Last known openpilot version on device
    pub sim_id: Option<String>,               // Last known sim_id of SIM in device
}

/// ## Device location
/// GET /v1/devices/:dongle_id/location
///
#[derive(Serialize, Debug, Default)]
pub struct DeviceLocationResponse {
    pub dongle_id: String,  // see Dongle ID
    pub lat: f64,           // Latitude, degrees
    pub lng: f64,           // Longitude, degrees
    pub time: i64,          // Milliseconds since epoch
    pub accuracy: f64,      // Accuracy (m)
    pub speed: f64,         // Speed (m/s)
    pub bearing: f64,       // Direction in degrees from north
}

#[derive(Serialize, Debug, Default)]
pub struct DeviceStats {
    pub distance: f32,
    pub minutes: i32,
    pub routes: u32,
}

/// ## Device driving statistics
/// GET /v1.1/devices/:dongle_id/stats
///
/// Returns aggregate driving statistics for a device
#[derive(Serialize, Debug, Default)]
pub struct DeviceStatsResponse {
    pub all: DeviceStats,
    pub week: DeviceStats,
}

#[derive(Serialize, Debug, Default)]
pub struct DeviceUser {
    pub email: String,
    pub permission: String,
}

/// ## Device users
/// GET /v1/devices/:dongle_id/users
///
/// List users with access to a device
#[derive(Serialize, Debug, Default)]
pub struct DeviceUsersResponse {
    pub users: Vec<DeviceUser>
}

#[derive(Serialize, Deserialize, Default)]
pub struct RouteSegment {
    pub can: bool,
    pub devicetype: u8,
    pub dongle_id: String,
    pub end_lat: f64,
    pub end_lng: f64,
    pub end_time: String,
    pub end_time_utc_millis: u64,
    pub fullname: String,
    pub hpgps: bool,
    pub init_logmonotime: u64,
    pub is_preserved: bool,
    pub is_public: bool,
    pub length: f64,
    pub passive: bool,
    pub platform: String,
    pub maxcamera: i32,
    pub segment_end_times: Vec<u64>,
    pub segment_numbers: Vec<u32>,
    pub segment_start_times: Vec<u64>,

    pub url: String,
    pub user_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct RouteSegmentResponse {
    pub segments: Vec<RouteSegment>,
}

/// "email": "commaphone3@gmail.com",
/// 
/// "id": "2e9eeac96ea4e6a6",
/// 
/// "points": 34933,
/// 
/// "regdate": 1465103707,
/// 
/// "superuser": false,
/// 
/// "username": "joeyjoejoe"
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MeResponse {
    pub email: Option<String>,
    pub id: String,
    pub points: i64,
    pub regdate: i64,
    pub superuser: bool,
    pub username: String,
}

/// "alias": "Comma EON",
/// 
/// "athena_host": "prod-comma-public-athena-0.prod-comma-public-athena.production.svc.cluster.local",
/// 
/// "device_type": "neo",
/// 
/// "dongle_id": "4bba516fb4439b31",
/// 
/// "ignore_uploads": null,
/// 
/// "is_owner": true,
/// 
/// "is_paired": true,
/// 
/// "last_athena_ping": 1644418781,
/// 
/// "last_gps_accuracy": 12,
/// 
/// "last_gps_bearing": 0,
/// 
/// "last_gps_lat": 32.0,
/// 
/// "last_gps_lng": -117.0,
/// 
/// "last_gps_speed": 0,
/// 
/// "last_gps_time": 1558583671000,
/// 
/// "openpilot_version": "0.8.13",
/// 
/// "prime": true,
/// 
/// "prime_type": 1,
/// 
/// "public_key": "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQDLI++RMXz29fBPNdECbCL8SGQ1+O/y1xhLsm/XsApxlghsKiWFSXiLJbHkHFOhQb6F421pVMZI0NtVXK5hUmwaAYLVg644/sLv/J32iW2vvdntT6GRTJxJr4LvbuXuBggW2sIYINFOOKng71CO5BxUNn+WNmeYFSqblFi4HjIuGbUZABuF9t0nkjMMVDZm9pTeeWqJtC4BxlACmJPA/88bdsiq4VDZ51yWqXxKJAq1HpG8RXpBs2leNQfnqF/mwtAkSeatqJYTjNAv77lFVg0rOQ6XjDLGdtRiloD+mNnJa1CJF4NiUG7hY/mdmolE4ML9W8YYX1aHNROmZApAt+Bn root@localhost\n",
/// 
/// "serial": "fa9bfe8a",
/// 
/// "sim_id": "890000000000000000",
/// 
/// "trial_claimed": false
/// 

#[derive(Serialize, Deserialize, Debug)]
pub struct DeviceFeatures {
    nav: bool,
    prime: bool,
    prime_data: bool,
}

impl Default for DeviceFeatures {
    fn default() -> Self {
        Self {
            nav: true,
            prime: true,
            prime_data: false,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct DeviceResponse {
    pub alias: String,
    pub athena_host: String,
    pub device_type: String,
    pub dongle_id: String,
    pub ignore_uploads: bool,
    pub is_owner: bool,
    pub is_paired: bool,
    pub last_athena_ping: i64,
    pub last_gps_accuracy: f64,
    pub last_gps_bearing: f64,
    pub last_gps_lat: f64,
    pub last_gps_lng: f64,
    pub last_gps_speed: f64,
    pub last_gps_time: i64,
    pub openpilot_version: String,
    pub prime: bool,
    pub prime_type: i16,
    pub public_key: String,
    pub serial: String,
    pub sim_id: Option<String>,
    pub trial_claimed: bool,
    pub online: bool,
    pub eligible_features: DeviceFeatures,

}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MyDevicesResponse {
    devices: Vec<DeviceResponse>
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct FilesResponse {
    pub cameras: Vec<String>,
    pub dcameras: Vec<String>,
    pub ecameras: Vec<String>,
    pub logs: Vec<String>,
    pub qcameras: Vec<String>,
    pub qlogs: Vec<String>,
}

#[derive(Serialize, Debug, Default)]
pub struct UnPairResponse {
    pub success: bool,
}