use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Sements::Table)
                    //.col(pk_auto(Sements::Id))
                    .col(string_uniq(Sements::CanonicalName).unique_key())
                    .col(string(Sements::CanonicalRouteName))
                    .col(string(Sements::Url))
                    .col(string(Sements::QlogUrl))
                    .col(string(Sements::RlogUrl))
                    .col(string(Sements::FcamUrl))
                    .col(string(Sements::DcamUrl))
                    .col(string(Sements::EcamUrl))
                    //.col(string_null(Sements::GitRemote))
                    .col(big_integer(Sements::StartTimeUtcMillis))
                    .col(integer_null(Sements::CreateTime))
                    .col(boolean_null(Sements::Hpgps))
                    .col(big_integer(Sements::EndTimeUtcMillis))
                    .col(float_null(Sements::EndLng))
                    .col(float_null(Sements::StartLng))
                    .col(boolean_null(Sements::Passive))
                    .col(tiny_integer(Sements::ProcLog))
                    //.col(string_null(Sements::Version))
                    .col(string_null(Sements::GitBranch))
                    .col(float_null(Sements::EndLat))
                    .col(tiny_integer_null(Sements::ProcCamera))
                    //.col(tiny_integer_null(Sements::Devicetype))
                    //.col(boolean_null(Sements::GitDirty))
                    
                    .col(boolean(Sements::Can))
                    //.col(string(Sements::GitCommit))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-segments-routes")
                            .from(Sements::Table, Sements::CanonicalRouteName)
                            .to(Routes::Table, Routes::CanonicalRouteName)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Sements::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Sements {
    Table,
    CanonicalName,
    CanonicalRouteName,
    Url,
    QlogUrl,
    RlogUrl,
    FcamUrl,
    DcamUrl,
    EcamUrl,
    //GitRemote,
    StartTimeUtcMillis,
    CreateTime,
    Hpgps,
    EndTimeUtcMillis,
    EndLng,
    StartLng,
    Passive,
    ProcLog,
    //Version,
    GitBranch,
    EndLat,
    ProcCamera,
    //Devicetype,
    //GitDirty,
    Can,
    //GitCommit,
    
}

#[derive(DeriveIden)]
enum Routes {
    Table,
    CanonicalRouteName,
}
