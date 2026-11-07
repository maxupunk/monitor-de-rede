//! O esquema inteiro do banco principal (ADR 017, Fase 13): o do núcleo e o de
//! cada área, que cada crate leva consigo. O núcleo vem primeiro porque as
//! tabelas das áreas só têm FK para as dele; entre áreas não há FK.

use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        [
            netmonitor_core::migrations::migrations(),
            netmonitor_network::migrations::migrations(),
            netmonitor_network_wifi::migrations::migrations(),
            netmonitor_network_link_tests::migrations::migrations(),
            netmonitor_servers::migrations::migrations(),
            netmonitor_vpn::migrations::migrations(),
            netmonitor_assistant::migrations::migrations(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}
