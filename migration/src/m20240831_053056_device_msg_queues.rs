use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(DeviceMsgQueues::Table)
                    .col(pk_auto(DeviceMsgQueues::Id))
                    .col(string(DeviceMsgQueues::DongleId))
                    .col(json(DeviceMsgQueues::JsonRpcRequest))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-device_msg_queues-devices")
                            .from(DeviceMsgQueues::Table, DeviceMsgQueues::DongleId)
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
            .drop_table(Table::drop().table(DeviceMsgQueues::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum DeviceMsgQueues {
    Table,
    Id,
    DongleId,
    JsonRpcRequest,
}

#[derive(DeriveIden)]
enum Devices {
    Table,
    DongleId,
}
