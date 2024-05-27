use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(Routes::Table)
                    .col(string_uniq(Routes::Fullname).unique_key().primary_key())
                    .col(string_null(Routes::GitRemote))
                    .col(string_null(Routes::Version))
                    .col(string_null(Routes::GitBranch))
                    .col(timestamp_null(Routes::StartTime))
                    .col(string(Routes::Platform))
                    .col(boolean(Routes::Public).default(false))
                    .col(tiny_integer_null(Routes::Devicetype))
                    .col(boolean_null(Routes::GitDirty))
                    .col(string(Routes::Url))
                    .col(boolean(Routes::Can))
                    .col(string_null(Routes::GitCommit))
                    .col(string(Routes::DeviceDongleId))
                    .col(big_integer(Routes::CreateTime))
                    .col(double(Routes::StartLat))
                    .col(double(Routes::StartLng))
                    .col(double(Routes::EndLat))
                    .col(double(Routes::EndLng))
                    .col(timestamp_null(Routes::EndTime))
                    .col(big_integer(Routes::EndTimeUtcMillis))
                    .col(boolean(Routes::Hpgps))
                    .col(big_integer(Routes::InitLogmonotime))
                    .col(boolean(Routes::IsPreserved))
                    .col(boolean(Routes::IsPublic))
                    .col(float(Routes::Length))
                    .col(integer(Routes::Maxcamera))
                    .col(integer(Routes::Maxdcamera))
                    .col(integer(Routes::Maxecamera))
                    .col(integer(Routes::Maxlog))
                    .col(integer(Routes::Maxqlog))
                    .col(integer(Routes::Maxqcamera))
                    .col(boolean(Routes::Passive))
                    .col(integer(Routes::Proccamera))
                    .col(integer(Routes::Proclog))
                    .col(integer(Routes::Procqcamera))
                    .col(integer(Routes::Procqlog))
                    .col(boolean(Routes::Radar))
                    .col(string_null(Routes::Rating))
                    .col(json(Routes::SegmentEndTimes))
                    .col(json(Routes::SegmentNumbers))
                    .col(json(Routes::SegmentStartTimes))
                    .col(string(Routes::ShareExp))
                    .col(string(Routes::ShareSig))
                    .col(big_integer(Routes::StartTimeUtcMillis))
                    .col(string(Routes::UserId))
                    .col(string(Routes::Vin))
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
pub enum Routes {
    Table,
    StartTime,
    Platform,
    Public,
    GitRemote,
    Version,
    GitBranch,
    Devicetype,
    GitDirty,
    Url,
    Can,
    GitCommit,
    DeviceDongleId,
    CreateTime,
    EndLat,
    EndLng,
    EndTime,
    EndTimeUtcMillis,
    Fullname,
    Hpgps,
    InitLogmonotime,
    IsPreserved,
    IsPublic,
    Length,
    Maxcamera,
    Maxdcamera,
    Maxecamera,
    Maxlog,
    Maxqlog,
    Maxqcamera,
    Passive,
    Proccamera,
    Proclog,
    Procqcamera,
    Procqlog,
    Radar,
    Rating,
    SegmentEndTimes,
    SegmentNumbers,
    SegmentStartTimes,
    ShareExp,
    ShareSig,
    StartLat,
    StartLng,
    StartTimeUtcMillis,
    UserId,
    Vin,
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    DongleId,
}
