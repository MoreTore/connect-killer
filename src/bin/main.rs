use loco_rs::cli;
use connect::app::App;
use migration::Migrator;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> loco_rs::Result<()> {
    cli::main::<App, Migrator>().await
}