use std::collections::HashMap;
use std::path::Path;
use std::env;
use std::sync::{Arc, Mutex};
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
    storage,
    config::Config,
};
use migration::{Migrator};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::{
    controllers, initializers,
    models::_entities::{devices, notes, routes, segments, users},
    tasks,
    workers::downloader::DownloadWorker,
};

use reqwest::{Client};
use axum::Extension;
use tower_http::normalize_path::NormalizePathLayer;
use tower_layer::Layer;
use tokio::sync::{mpsc, oneshot, RwLock};

#[derive(Serialize, Deserialize, Clone)]
struct JsonRpcCommand {
    id: String,
    method: String,
    params: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
struct JsonRpcResponse {
    id: String,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
}

type SharedState = Arc<RwLock<App>>;

use crate::controllers::ws::ConnectionManager;

pub struct App {
    command_sender: mpsc::Sender<JsonRpcCommand>,
    pending_commands: Arc<Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>>,
    offline_queues: Arc<Mutex<HashMap<String, Vec<JsonRpcCommand>>>>, // offline queue for each device
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
        create_app::<Self, Migrator>(mode, environment).await
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(
            initializers::view_engine::ViewEngineInitializer,
        )])
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
            .add_route(controllers::ws::routes())
            .add_route(controllers::v2::routes())
            .add_route(controllers::useradmin::routes())
            .add_route(controllers::connectincomming::routes())
            .add_route(controllers::connectdata::routes())
            .add_route(controllers::v1::routes())
            .add_route(controllers::notes::routes())
            .add_route(controllers::auth::routes())
            .add_route(controllers::user::routes())
    }

    fn connect_workers<'a>(p: &'a mut Processor, ctx: &'a AppContext) {
        p.register(crate::workers::bootlog_parser::BootlogParserWorker::build(ctx));
        p.register(crate::workers::jpg_extractor::JpgExtractorWorker::build(ctx));
        p.register(crate::workers::log_parser::LogSegmentWorker::build(ctx));
        p.register(DownloadWorker::build(ctx));
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks.register(tasks::collect_data::CollectData);
        tasks.register(tasks::seed_from_mkv::SeedFromMkv);
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
    async fn after_routes(router: axum::Router, ctx: &AppContext) -> Result<axum::Router> {
        let (command_sender, command_receiver) = mpsc::channel(100);
        
        let shared_state = Arc::new(RwLock::new(App {
            command_sender,
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
            offline_queues: Arc::new(Mutex::new(HashMap::new())),
        }));

        let client = Client::new();
        let manager: Arc<ConnectionManager> = ConnectionManager::new();

        crate::controllers::ws::send_ping_to_all_devices(manager.clone());

        let router = router
            .layer(NormalizePathLayer::trim_trailing_slash())
            .layer(Extension(client))
            .layer(Extension(manager))
            .layer(Extension(shared_state));

        Ok(router)
    }

    async fn storage(
        _config: &Config,
        _environment: &Environment,
    ) -> Result<Option<storage::Storage>> {
        // get the project root directory
        //let root = env::current_dir().expect("Failed to get current directory");
        
        let local_storage = storage::Storage::single(storage::drivers::local::new_with_prefix("uploads")
            .expect("Failed to create local storage driver"));
        return Ok(Some(local_storage));
    }
    

}