use backend::app::App;
use backend::migrator::Migrator;
use loco_rs::cli;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    cli::main::<App, Migrator>().await
}
