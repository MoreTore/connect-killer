use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Devices::Table)
                    .col(string(Devices::DongleId).unique_key().primary_key())
                    .col(string(Devices::Serial))
                    .col(string(Devices::Imei))
                    .col(string(Devices::Imei2))
                    .col(string(Devices::PublicKey).unique_key())
                    .col(string_null(Devices::SimId))
                    .col(boolean(Devices::Prime))
                    .col(tiny_integer(Devices::PrimeType))
                    .col(boolean(Devices::Online))
                    .col(big_integer(Devices::LastAthenaPing))
                    .col(boolean_null(Devices::UploadsAllowed))
                    .col(integer_null(Devices::OwnerId))
                    .col(string(Devices::DeviceType))
                    .col(string(Devices::Alias))
                    .col(big_integer_null(Devices::ServerStorage))
                    .col(json_null(Devices::Locations))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-devices-users")
                            .from(Devices::Table, Devices::OwnerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Devices::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    DongleId,
    PublicKey,
    Serial,
    Imei,
    Imei2,
    SimId,
    Prime,
    PrimeType,
    Online,
    LastAthenaPing,
    UploadsAllowed,
    OwnerId,
    DeviceType,
    Alias,
    ServerStorage,
    Locations,
}


#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
