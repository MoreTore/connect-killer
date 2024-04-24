use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Devices::Table)
                    .col(pk_auto(Devices::Id))
                    .col(string_null(Devices::DongleId).unique_key())
                    .col(string_null(Devices::Serial))
                    .col(string_null(Devices::SimId))
                    .col(boolean_null(Devices::Prime))
                    .col(tiny_integer_null(Devices::Primetype))
                    .col(string_null(Devices::LastPing))
                    .col(boolean_null(Devices::UploadsAllowed))
                    .col(integer(Devices::OwnerId))
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
    Id,
    DongleId,
    Serial,
    SimId,
    Prime,
    Primetype,
    LastPing,
    UploadsAllowed,
    OwnerId,
    
}


#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
