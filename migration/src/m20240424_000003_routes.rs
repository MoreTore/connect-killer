use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Routes::Table)
                    .col(string_uniq(Routes::CanonicalRouteName).unique_key().primary_key())
                    .col(string_null(Routes::GitRemote))
                    .col(string_null(Routes::Version))
                    .col(string_null(Routes::GitBranch))
                    .col(tiny_integer_null(Routes::Devicetype))
                    .col(boolean_null(Routes::GitDirty))
                    .col(string(Routes::Url))
                    .col(boolean(Routes::Can))
                    .col(string_null(Routes::GitCommit))
                    .col(string(Routes::DeviceDongleId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-routes-devices")
                            .from(Routes::Table, Routes::DeviceDongleId)
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
    DeviceDongleId,
}


#[derive(DeriveIden)]
enum Devices {
    Table,
    DongleId,
}
