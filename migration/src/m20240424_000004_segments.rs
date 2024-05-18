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
                    .col(string_uniq(Segments::CanonicalName).unique_key().primary_key())
                    .col(string(Segments::CanonicalRouteName))
                    .col(small_integer(Segments::Number))
                    .col(string(Segments::Url))
                    .col(string(Segments::UlogUrl))
                    .col(string(Segments::QlogUrl))
                    .col(string(Segments::QcamUrl))
                    .col(string(Segments::RlogUrl))
                    .col(string(Segments::FcamUrl))
                    .col(string(Segments::DcamUrl))
                    .col(string(Segments::EcamUrl))
                    .col(integer(Segments::Proccamera).default(0))
                    .col(integer(Segments::Proclog).default(0))
                    .col(boolean(Segments::Can).default(false))
                    .col(boolean(Segments::Hpgps).default(false))
                    .col(double(Segments::StartLng).default(0.0))
                    .col(double(Segments::EndLng).default(0.0))
                    .col(double(Segments::StartLat).default(0.0))
                    .col(double(Segments::EndLat).default(0.0))
                    //.col(string_null(Segments::GitRemote))
                    .col(big_integer(Segments::StartTimeUtcMillis))
                    .col(big_integer(Segments::CreateTime))
                    .col(big_integer(Segments::EndTimeUtcMillis))
                    .col(boolean_null(Segments::Passive).default(true))
                    //.col(string_null(Segments::Version))
                    .col(string_null(Segments::GitBranch))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-segments-routes")
                            .from(Segments::Table, Segments::CanonicalRouteName)
                            .to(Routes::Table, Routes::Fullname)
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
pub enum Segments {
    Table,
    CanonicalName,
    CanonicalRouteName,
    Number,
    Url,
    UlogUrl,
    QlogUrl,
    QcamUrl,
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
    StartLat,
    EndLat,
    Passive,
    Proclog,
    //Version,
    GitBranch,
    Proccamera,
    //Devicetype,
    //GitDirty,
    Can,
    //GitCommit,
    
}

#[derive(DeriveIden)]
enum Routes {
    Table,
    Fullname,
}
