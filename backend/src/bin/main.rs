use backend::app::App;
use backend::migrator::Migrator;
use loco_rs::cli;

// O runtime é montado à mão para os segredos `<VAR>_FILE` entrarem no
// ambiente antes de existir qualquer thread (`secret_files`).
fn main() -> loco_rs::Result<()> {
    backend::secret_files::load();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(cli::main::<App, Migrator>())
}
