use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = table_auto(Users::Table)
            .col(pk_auto(Users::Id))
            .col(uuid(Users::Identity))
            .col(string_null(Users::Email))
            .col(string(Users::Name))
            .col(big_integer(Users::Points))
            .col(boolean(Users::Superuser))
            //.col(json_null(Users::Locations))
            .to_owned();
        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Users {
    Table,
    Id,
    Identity,
    Email,
    Name,
    Superuser,
    Points,
    //Locations,
}
