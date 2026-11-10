//! Tabelas do registry (submódulo da área Servidores). Só têm FK para tabelas
//! do núcleo (`users`) e para as próprias.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20261109_000001_registry;

/// As migrations deste submódulo.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261109_000001_registry::Migration)]
}
