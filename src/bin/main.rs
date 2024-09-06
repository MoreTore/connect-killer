use loco_rs::cli;
use connect::app::App;
use migration::Migrator;

#[tokio::main(worker_threads = 24)]
async fn main() -> loco_rs::Result<()> {
    cli::main::<App, Migrator>().await
}