use std::collections::HashMap;

use std::path::Path;
use std::env;

use std::sync::{Arc, Mutex};
use tokio::time::{self};
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
use migration::{Migrator};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::{
    controllers, initializers,
    models::_entities::{devices, notes, users},
    tasks,
};

use reqwest::{Client};
use tower_http::normalize_path::NormalizePathLayer;
use tower_layer::Layer;
use tokio::sync::{mpsc, oneshot, RwLock};

use axum::{
    extract::Host,
    handler::HandlerWithoutStateExt,
    http::{StatusCode, Uri},
    response::Redirect, Extension,
};
use axum_server::tls_rustls::RustlsConfig;
use std::{net::SocketAddr, path::PathBuf};


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

    fn routes(ctx: &AppContext) -> AppRoutes {
        if ctx.environment == loco_rs::environment::Environment::Any("connect".to_string()) { 
            return AppRoutes::with_default_routes();
        }
        AppRoutes::with_default_routes()
            .add_route(controllers::ws::routes())
            .add_route(controllers::v2::routes())
            .add_route(controllers::useradmin::routes())
            .add_route(controllers::connectincomming::routes())
            .add_route(controllers::connectdata::routes())
            .add_route(controllers::v1::routes())
    }

    fn connect_workers<'a>(p: &'a mut Processor, ctx: &'a AppContext) {
        if ctx.environment == loco_rs::environment::Environment::Any("connect".to_string()) {
            return;
        }
        p.register(crate::workers::bootlog_parser::BootlogParserWorker::build(ctx));
        p.register(crate::workers::jpg_extractor::JpgExtractorWorker::build(ctx));
        p.register(crate::workers::log_parser::LogSegmentWorker::build(ctx));
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks.register(tasks::huggingface::Huggingface);
        tasks.register(tasks::deleter::Deleter);
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
        //db::seed::<routes::ActiveModel>(db, &base.join("routes.yaml").display().to_string()).await?;
        //db::seed::<segments::ActiveModel>(db, &base.join("segments.yaml").display().to_string()).await?;
        Ok(())
    }
    async fn after_routes(router: axum::Router, ctx: &AppContext) -> Result<axum::Router> {
        let router = NormalizePathLayer::trim_trailing_slash().layer(router);
        let router = axum::Router::new().nest_service("", router);
        
        if ctx.environment == loco_rs::environment::Environment::Any("connect".to_string()) {
            return Ok(router);
        }

        let manager: Arc<ConnectionManager> = ConnectionManager::new();
        let ping_manager = manager.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(time::Duration::from_secs(10)); // Ping every 10 seconds
            loop {
                interval.tick().await;
                crate::controllers::ws::send_ping_to_all_devices(ping_manager.clone()).await; 
            }
        });
        

        let (command_sender, _command_receiver) = mpsc::channel(100);
        let shared_state = Arc::new(RwLock::new(App {
            command_sender,
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
            offline_queues: Arc::new(Mutex::new(HashMap::new())),
        }));
        let client = Client::new();

        let router = router
            .layer(Extension(client))
            .layer(Extension(manager))
            .layer(Extension(shared_state));

        Ok(router)
    }

    async fn serve(app: axum::Router, server_config: loco_rs::boot::ServeParams) -> Result<()> {
        let my_server_config = MyServerConfig {
            http: server_config.port as u16,
            https: (server_config.port + 111) as u16,
            binding: server_config.binding,
        };
        //tokio::spawn(redirect_http_to_https(my_server_config.clone()));
        // configure certificate and private key used by https
        let config = RustlsConfig::from_pem_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("self_signed_certs")
                .join("cert.pem"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("self_signed_certs")
                .join("key.pem"),
        )
        .await
        .unwrap();
    // Clone the app for the HTTP server
    let app_clone = app.clone();
    // HTTPS Listener
    let https_addr = SocketAddr::from((
        std::net::Ipv6Addr::UNSPECIFIED, 
        my_server_config.https
    ));

    let https_server = tokio::spawn(async move {
        axum_server::bind_rustls(https_addr, config)
            .serve(app.into_make_service())
            .await
    });

    // HTTP Listener
    let http_addr = SocketAddr::from((
        std::net::Ipv6Addr::UNSPECIFIED,
        my_server_config.http
    ));

    let http_server = tokio::spawn(async move {
        axum_server::bind(http_addr)
            .serve(app_clone.into_make_service())
            .await
    });

    // Await both servers separately
    if let Err(e) = http_server.await {
        eprintln!("HTTP server failed: {}", e);
    }

    if let Err(e) = https_server.await {
        eprintln!("HTTPS server failed: {}", e);
    }

    Ok(())
    }
}

#[derive(Clone)]
struct MyServerConfig {
    http: u16,
    https: u16,
    binding: String
}

async fn redirect_http_to_https(my_server_config: MyServerConfig) {
    let config_clone = my_server_config.clone(); // Clone the config for the closure

    fn make_https(host: String, uri: Uri, my_server_config: MyServerConfig) -> Result<Uri, Box<dyn std::error::Error>> {
        let mut parts = uri.into_parts();
        parts.scheme = Some(axum::http::uri::Scheme::HTTPS.try_into()?);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host = host.replace(&my_server_config.http.to_string(), &my_server_config.https.to_string());
        parts.authority = Some(https_host.parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, config_clone.clone()) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", my_server_config.binding, my_server_config.http)).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, redirect.into_make_service())
        .await
        .unwrap();
}
