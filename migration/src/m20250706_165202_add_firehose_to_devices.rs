use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Devices {
    Table,
    Firehose,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        //
        // add column
        //
        
        manager
            .alter_table(
                Table::alter()
                    .table(Devices::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Devices::Firehose)
                            .boolean()
                            .default(true),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Devices::Table)
                    .drop_column(Devices::Firehose)
                    .to_owned(),
            )
            .await
    }
}

