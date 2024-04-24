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
                    .col(string_null(Devices::DongleId))
                    .col(integer(Devices::UserId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-devices-users")
                            .from(Devices::Table, Devices::UserId)
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
    UserId,
    
}


#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
