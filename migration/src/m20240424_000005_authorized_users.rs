use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                table_auto(AuthorizedUsers::Table)
                    .primary_key(
                        Index::create()
                            .name("idx-authorized_users-refs-pk")
                            .table(AuthorizedUsers::Table)
                            .col(AuthorizedUsers::UserId)
                            .col(AuthorizedUsers::DeviceDongleId),
                    )
                    .col(integer(AuthorizedUsers::UserId))
                    .col(integer(AuthorizedUsers::DeviceDongleId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-authorized_users-users")
                            .from(AuthorizedUsers::Table, AuthorizedUsers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-authorized_users-devices")
                            .from(AuthorizedUsers::Table, AuthorizedUsers::DeviceDongleId)
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
            .drop_table(Table::drop().table(AuthorizedUsers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuthorizedUsers {
    Table,
    UserId,
    DeviceDongleId,
    
}


#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Devices {
    Table,
    DongleId,
}
