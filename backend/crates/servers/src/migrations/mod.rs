//! Tabelas da Servidores (ADR 017, Fase 13). Só têm FK para tabelas do
//! núcleo; o ponto de montagem as junta depois das do núcleo.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20261007_000001_database_connections;
mod m20261108_000001_deploys;

/// As migrations da área, na ordem de criação.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20261007_000001_database_connections::Migration),
        Box::new(m20261108_000001_deploys::Migration),
    ]
}

#[cfg(test)]
netmonitor_core::area_test_migrator!(TestMigrator, migrations);
