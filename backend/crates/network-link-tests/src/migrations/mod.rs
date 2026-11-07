//! Tabelas dos testes de enlace (submódulo da área Rede). Só têm FK para tabelas do
//! núcleo; o ponto de montagem as junta depois das da base da área.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20261016_000001_link_tests;

/// As migrations deste submódulo.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261016_000001_link_tests::Migration)]
}

#[cfg(test)]
netmonitor_core::area_test_migrator!(
    TestMigrator,
    netmonitor_network::migrations::migrations,
    migrations
);
