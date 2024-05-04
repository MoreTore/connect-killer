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
        ]
    }
}