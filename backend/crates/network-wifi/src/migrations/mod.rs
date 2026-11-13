//! Tabelas do Wi-Fi (submódulo da área Rede). Só têm FK para tabelas do
//! núcleo; o ponto de montagem as junta depois das da base da área.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20261027_000001_wifi_watched_clients;
mod m20261113_000001_wifi_roaming_events;

/// As migrations deste submódulo.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20261027_000001_wifi_watched_clients::Migration),
        Box::new(m20261113_000001_wifi_roaming_events::Migration),
    ]
}

#[cfg(test)]
netmonitor_core::area_test_migrator!(
    TestMigrator,
    netmonitor_network::migrations::migrations,
    migrations
);
