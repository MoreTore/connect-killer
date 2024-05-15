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
                    .col(big_integer(Routes::StartTime))
                    .col(float(Routes::Miles))
                    .col(small_integer(Routes::MaxQlog))
                    .col(small_integer(Routes::MaxQcam))
                    .col(string(Routes::Platform))
                    .col(boolean(Routes::Public).default(false))
                    .col(tiny_integer_null(Routes::Devicetype))
                    .col(boolean_null(Routes::GitDirty))
                    .col(string(Routes::Url))
                    .col(boolean(Routes::Can))
                    .col(string_null(Routes::GitCommit))
                    .col(string(Routes::DeviceDongleId))
                    .col(big_integer(Routes::CreateTime))
                    .col(float(Routes::EndLat))
                    .col(float(Routes::EndLng))
                    .col(string(Routes::EndTime))
                    .col(big_integer(Routes::EndTimeUtcMillis))
                    .col(string(Routes::Fullname))
                    .col(boolean(Routes::Hpgps))
                    .col(big_integer(Routes::InitLogmonotime))
                    .col(boolean(Routes::IsPreserved))
                    .col(boolean(Routes::IsPublic))
                    .col(float(Routes::Length))
                    .col(integer(Routes::MaxCamera))
                    .col(integer(Routes::MaxDcamera))
                    .col(integer(Routes::MaxEcamera))
                    .col(integer(Routes::MaxLog))
                    .col(integer(Routes::MaxQcamera))
                    .col(boolean(Routes::Passive))
                    .col(integer(Routes::ProcCamera))
                    .col(integer(Routes::ProcLog))
                    .col(integer(Routes::ProcQcamera))
                    .col(integer(Routes::ProcQlog))
                    .col(boolean(Routes::Radar))
                    .col(string_null(Routes::Rating))
                    .col(json(Routes::SegmentEndTimes))
                    .col(json(Routes::SegmentNumbers))
                    .col(json(Routes::SegmentStartTimes))
                    .col(string(Routes::ShareExp))
                    .col(string(Routes::ShareSig))
                    .col(float(Routes::StartLat))
                    .col(float(Routes::StartLng))
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
    CanonicalRouteName,
    StartTime,
    Platform,
    Miles,
    MaxQcam,
    MaxQlog,
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
    MaxCamera,
    MaxDcamera,
    MaxEcamera,
    MaxLog,
    MaxQcamera,
    Passive,
    ProcCamera,
    ProcLog,
    ProcQcamera,
    ProcQlog,
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
