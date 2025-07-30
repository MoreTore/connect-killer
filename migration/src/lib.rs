#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod m20220101_000001_users;
mod m20240424_000002_devices;
pub mod m20240424_000003_routes;
pub mod m20240424_000004_segments;
mod m20231103_114510_notes;
mod m20240425_071518_authorized_users;
mod m20240504_174513_bootlogs;
mod m20240522_001958_add_segment_miles;
mod m20240522_001958_add_route_miles;
mod m20240622_195829_anonlogs;
mod m20240826_165428_add_devices_storage;
//mod m20240831_002142_add_users_locations;
mod m20240831_010827_add_devices_locations;
mod m20240831_053056_device_msg_queues;
mod m20250706_165202_add_firehose_to_devices;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20231103_114510_notes::Migration),
            Box::new(m20240424_000002_devices::Migration),
            Box::new(m20240424_000003_routes::Migration),
            Box::new(m20240424_000004_segments::Migration),
            
            Box::new(m20240425_071518_authorized_users::Migration),
            Box::new(m20240504_174513_bootlogs::Migration),
            //Box::new(m20240522_001958_add_route_miles::Migration),
            //Box::new(m20240522_001958_add_segment_miles::Migration),
            Box::new(m20240622_195829_anonlogs::Migration),
            Box::new(m20240826_165428_add_devices_storage::Migration),
            //Box::new(m20240831_002142_add_users_locations::Migration),
            Box::new(m20240831_010827_add_devices_locations::Migration),
            Box::new(m20240831_053056_device_msg_queues::Migration),
            Box::new(m20250706_165202_add_firehose_to_devices::Migration),
        ]
    }
}