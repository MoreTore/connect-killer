use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Bootlogs::Table)
                    .col(pk_auto(Bootlogs::Id))
                    .col(date_time(Bootlogs::Datetime))
                    .col(string(Bootlogs::DongleId))
                    .col(string_null(Bootlogs::Url))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-bootlogs-devices")
                            .from(Bootlogs::Table, Bootlogs::DongleId)
                            .to(Devices::Table, Devices::DongleId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Bootlogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Bootlogs {
    Table,
    Id,
    Url,
    DongleId,
    Datetime,
    
}


#[derive(DeriveIden)]
enum Devices {
    Table,
    DongleId,
}
