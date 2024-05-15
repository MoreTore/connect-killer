use serde::Serialize;

/// ## Device Info Response
/// GET /v1.1/devices/:dongle_id/
/// 
#[derive(Serialize, Debug, Default)]
pub struct DeviceInfoResponse {
    pub dongle_id: String,            // see Dongle ID
    pub alias: String,                // Globally unique device nickname
    pub serial: String,               // Device serial
    pub athena_host: String,          // Last connected athena server
    pub last_athena_ping: i64,        // (integer)	Timestamp of last athena ping
    pub ignore_uploads: bool,         // If uploads are ignored
    pub is_paired: bool,              // Device has an owner
    pub is_owner: bool,               // Authed user has write-access to device
    pub public_key: String,           // 2048-bit public RSA key
    pub prime: bool,                  // If device has prime
    pub prime_type: i16,              // Prime type: 0: no prime, 1: standard prime, 2: prime lite
    pub trial_claimed: bool,          // If device prime trial is claimed
    pub device_type: String,          // one of ("neo", "panda", "app")
    pub last_gps_time: i64,           // Milliseconds since epoch of last gps. Updates upon successful call to device location endpoint
    pub last_gps_lat: f64,            // Latitude of last location
    pub last_gps_lng: f64,            // Longitude of last location
    pub last_gps_accur: f64,          // Accuracy (m) of last location
    pub last_gps_speed: f64,          // Speed (m/s) at last location
    pub last_gps_bearing: f64,        // Direction of last location in degrees from north
    pub openpilot_version: String,    // Last known openpilot version on device
    pub sim_id: String,               // Last known sim_id of SIM in device
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
    pub distance: f64,
    pub minutes: i64,
    pub routes: i64,
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
