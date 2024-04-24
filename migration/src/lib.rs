#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod m20220101_000001_users;
mod m20231103_114510_notes;


mod m20240424_101126_devices;
mod m20240424_141802_sements;

mod m20240424_144603_routes;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20231103_114510_notes::Migration),
            Box::new(m20240424_101126_devices::Migration),
            Box::new(m20240424_141802_sements::Migration),
            Box::new(m20240424_144603_routes::Migration),
        ]
    }
}