use std::path::Path;

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    boot::{create_app, BootResult, StartMode},
    controller::AppRoutes,
    db::{self, truncate_table},
    environment::Environment,
    task::Tasks,
    worker::{AppWorker, Processor},
    Result,
};
use migration::Migrator;
use sea_orm::DatabaseConnection;

use crate::{
    controllers, initializers,
    models::_entities::{notes, users},
    tasks,
    workers::downloader::DownloadWorker,
};

use reqwest::{Body ,Client};
use axum::Extension;

pub struct App;
#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(mode: StartMode, environment: &Environment) -> Result<BootResult> {
        create_app::<Self, Migrator>(mode, environment).await
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(
            initializers::view_engine::ViewEngineInitializer,
        )])
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
            .add_route(controllers::connectincomming::routes())
            .add_route(controllers::connectdata::routes())
            .add_route(controllers::v1::routes())
            .add_route(controllers::notes::routes())
            .add_route(controllers::auth::routes())
            .add_route(controllers::user::routes())
    }

    fn connect_workers<'a>(p: &'a mut Processor, ctx: &'a AppContext) {
        p.register(DownloadWorker::build(ctx));
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks.register(tasks::seed::SeedData);
    }

    async fn truncate(db: &DatabaseConnection) -> Result<()> {
        truncate_table(db, users::Entity).await?;
        truncate_table(db, notes::Entity).await?;
        Ok(())
    }

    async fn seed(db: &DatabaseConnection, base: &Path) -> Result<()> {
        db::seed::<users::ActiveModel>(db, &base.join("users.yaml").display().to_string()).await?;
        db::seed::<notes::ActiveModel>(db, &base.join("notes.yaml").display().to_string()).await?;
        Ok(())
    }

    async fn after_routes(router: axum::Router, _ctx: &AppContext) -> Result<axum::Router> {
        // cache should reside at: ~/.cache/huggingface/hub
        let client = Client::new();
        Ok(router.layer(Extension(client)))
    }
}