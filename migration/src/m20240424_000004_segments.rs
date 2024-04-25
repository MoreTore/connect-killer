use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Segments::Table)
                    //.col(pk_auto(Segments::Id))
                    .col(string_uniq(Segments::CanonicalName).unique_key())
                    .col(string(Segments::CanonicalRouteName))
                    .col(string(Segments::Url))
                    .col(string(Segments::QlogUrl))
                    .col(string(Segments::RlogUrl))
                    .col(string(Segments::FcamUrl))
                    .col(string(Segments::DcamUrl))
                    .col(string(Segments::EcamUrl))
                    //.col(string_null(Segments::GitRemote))
                    .col(big_integer(Segments::StartTimeUtcMillis))
                    .col(integer_null(Segments::CreateTime))
                    .col(boolean_null(Segments::Hpgps))
                    .col(big_integer(Segments::EndTimeUtcMillis))
                    .col(float_null(Segments::EndLng))
                    .col(float_null(Segments::StartLng))
                    .col(boolean_null(Segments::Passive))
                    .col(tiny_integer(Segments::ProcLog))
                    //.col(string_null(Segments::Version))
                    .col(string_null(Segments::GitBranch))
                    .col(float_null(Segments::EndLat))
                    .col(tiny_integer_null(Segments::ProcCamera))
                    .col(boolean(Segments::Can))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-segments-routes")
                            .from(Segments::Table, Segments::CanonicalName)
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
            .drop_table(Table::drop().table(Segments::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Segments {
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
