//! Tabelas da Rede (ADR 017, Fase 13). Só têm FK para tabelas do
//! núcleo; o ponto de montagem as junta depois das do núcleo.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20260810_000013_discovery_runs;
mod m20260810_000014_discovery_results;
mod m20260930_000001_device_plugins;
mod m20261001_000001_device_plugin_installs;
mod m20261002_000001_plugin_fleet;
mod m20261005_000001_oui_vendors;
mod m20261016_000001_link_tests;
mod m20261027_000001_wifi_watched_clients;

/// As migrations da área, na ordem de criação.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260810_000013_discovery_runs::Migration),
        Box::new(m20260810_000014_discovery_results::Migration),
        Box::new(m20260930_000001_device_plugins::Migration),
        Box::new(m20261001_000001_device_plugin_installs::Migration),
        Box::new(m20261002_000001_plugin_fleet::Migration),
        Box::new(m20261005_000001_oui_vendors::Migration),
        Box::new(m20261016_000001_link_tests::Migration),
        Box::new(m20261027_000001_wifi_watched_clients::Migration),
    ]
}

#[cfg(test)]
netmonitor_core::area_test_migrator!(TestMigrator, migrations);
