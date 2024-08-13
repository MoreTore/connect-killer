use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Anonlogs::Table)
                    .col(pk_auto(Anonlogs::Id))
                    .col(string(Anonlogs::Url))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Anonlogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Anonlogs {
    Table,
    Id,
    Url,
    
}


