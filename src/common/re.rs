pub const DONGLE_ID: &str = r"[0-9a-z]{16}";
/// 
/// const MONOTONIC_TIMESTAMP: &str = r"[0-9a-f]{8}--[0-9a-f]{10}";
/// 
/// const TIMESTAMP: &str = r"[0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2}";
///
/// MONOTONIC_TIMESTAMP or TIMESTAMP
pub const ROUTE_NAME: &str = r"[0-9a-f]{8}--[0-9a-f]{10}|[0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2}";
/// Any number
pub const NUMBER: &str = r"[0-9]+";
pub const HEX: &str = r"[0-9a-f]+";
/// Any file name
pub const ANY_FILENAME: &str = r".+";
pub const ALLOWED_FILENAME: &str = r"(rlog\.(?:bz2|zst)|qlog\.(?:bz2|zst)|qcamera\.ts|fcamera\.hevc|dcamera\.hevc|ecamera\.hevc|qlog\.unlog|sprite\.jpg|coords\.json|events\.json)";
