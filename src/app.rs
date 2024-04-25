use std::path::Path;
use std::env;
use async_trait::async_trait;
use chrono::format;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    boot::{create_app, BootResult, StartMode},
    controller::AppRoutes,
    db::{self, truncate_table},
    environment::Environment,
    task::Tasks,
    worker::{AppWorker, Processor},
    Result,
    storage,
    config::Config,
};
use migration::{Migrator, RcOrArc};
use sea_orm::DatabaseConnection;

use crate::{
    controllers, initializers,
    models::_entities::{devices, notes, routes, segments, users},
    tasks,
    workers::downloader::DownloadWorker,
};

use reqwest::{Body ,Client};
use axum::Extension;

pub struct App {
    client: Client,
}
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
        let client = Client::new();
        let app = App { client };
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
        p.register(crate::workers::qlog_parser::QlogParserWorker::build(ctx));
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
        //db::seed::<notes::ActiveModel>(db, &base.join("notes.yaml").display().to_string()).await?;
        db::seed::<devices::ActiveModel>(db, &base.join("devices.yaml").display().to_string()).await?;
        db::seed::<routes::ActiveModel>(db, &base.join("routes.yaml").display().to_string()).await?;
        db::seed::<segments::ActiveModel>(db, &base.join("segments.yaml").display().to_string()).await?;
        Ok(())
    }

    async fn after_routes(router: axum::Router, _ctx: &AppContext) -> Result<axum::Router> {

        let client = Client::new();
        Ok(router.layer(Extension(client)))
    }

    async fn storage(
        _config: &Config,
        environment: &Environment,
    ) -> Result<Option<storage::Storage>> {
        // get the project root directory
        //let root = env::current_dir().expect("Failed to get current directory");
        
        let local_storage = storage::Storage::single(storage::drivers::local::new_with_prefix("uploads")
            .expect("Failed to create local storage driver"));
        return Ok(Some(local_storage));
    }

}