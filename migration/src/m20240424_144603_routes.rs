use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Routes::Table)
                    .col(string_uniq(Routes::CanonicalRouteName).unique_key())
                    .col(string_null(Routes::GitRemote))
                    .col(string_null(Routes::Version))
                    .col(string_null(Routes::GitBranch))
                    .col(tiny_integer_null(Routes::Devicetype))
                    .col(boolean_null(Routes::GitDirty))
                    .col(string(Routes::Url))
                    .col(boolean(Routes::Can))
                    .col(string_null(Routes::GitCommit))
                    .col(integer(Routes::DeviceId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-routes-devices")
                            .from(Routes::Table, Routes::DeviceId)
                            .to(Devices::Table, Devices::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Routes::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Routes {
    Table,
    CanonicalRouteName,
    GitRemote,
    Version,
    GitBranch,
    Devicetype,
    GitDirty,
    Url,
    Can,
    GitCommit,
    DeviceId,
    
}


#[derive(DeriveIden)]
enum Devices {
    Table,
    Id,
}
