//! Esquema do núcleo: as tabelas de base (inventário, monitores, alertas,
//! usuários, configurações, backup, rotinas) e o banco de logs ([`logs`]).
//!
//! Cada área leva as tabelas dela no próprio crate e o ponto de montagem junta
//! tudo num `Migrator` só (núcleo primeiro: as tabelas das áreas só têm FK para
//! as daqui). As entidades de todas as tabelas continuam em [`crate::models`].
//!
//! **Uma migration por tabela (ou grupo de tabelas), e só de criação.** O
//! sistema nunca foi instalado: mudar o esquema é editar a migration que cria a
//! tabela e recriar o banco de desenvolvimento — nada de migration de `ALTER`,
//! de dados ou de compatibilidade (DB-02). A ordem de criação, invertida, está
//! em `models::tables::CREATION_ORDER` para a limpeza entre testes.
//!
//! **`auth_tokens` não é criada.** A autenticação é `loco_rs::auth::JWT`,
//! stateless. O nome permanece no `CREATION_ORDER` para uma eventual adoção
//! futura de tokens opacos.

pub use sea_orm_migration::{MigrationTrait, MigratorTrait};

/// Migrator do banco de logs (`/data/logs.sqlite`), separado do principal.
pub mod logs;

mod m20220101_000001_users;
mod m20260810_000002_sites;
mod m20260810_000003_probes;
mod m20260810_000004_networks;
mod m20260810_000007_devices;
mod m20260810_000008_device_interfaces;
mod m20260810_000009_device_links;
mod m20260810_000010_monitors;
mod m20260810_000011_monitor_results;
mod m20260810_000012_metrics;
mod m20260810_000015_alert_rules;
mod m20260810_000016_alert_events;
mod m20260810_000019_dns_servers;
mod m20260810_000020_event_outbox;
mod m20260810_000021_probe_tasks;
mod m20260810_000022_system_settings;
mod m20260815_000003_notification_hygiene;
mod m20260821_000001_maintenance_windows;
mod m20260821_000002_monitor_results_hourly;
mod m20260821_000003_audit_logs;
mod m20260821_000004_push_subscriptions;
mod m20260922_000003_metrics_1m;
mod m20261006_000001_storage_destinations;
mod m20261008_000001_system_backup_plan;
mod m20261023_000001_scheduled_tasks;

/// As migrations do núcleo, na ordem de criação.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20220101_000001_users::Migration),
        Box::new(m20260810_000002_sites::Migration),
        Box::new(m20260810_000003_probes::Migration),
        Box::new(m20260810_000004_networks::Migration),
        Box::new(m20260810_000007_devices::Migration),
        Box::new(m20260810_000008_device_interfaces::Migration),
        Box::new(m20260810_000009_device_links::Migration),
        Box::new(m20260810_000010_monitors::Migration),
        Box::new(m20260810_000011_monitor_results::Migration),
        Box::new(m20260810_000012_metrics::Migration),
        Box::new(m20260810_000015_alert_rules::Migration),
        Box::new(m20260810_000016_alert_events::Migration),
        Box::new(m20260810_000019_dns_servers::Migration),
        Box::new(m20260810_000020_event_outbox::Migration),
        Box::new(m20260810_000021_probe_tasks::Migration),
        Box::new(m20260810_000022_system_settings::Migration),
        Box::new(m20260815_000003_notification_hygiene::Migration),
        Box::new(m20260821_000001_maintenance_windows::Migration),
        Box::new(m20260821_000002_monitor_results_hourly::Migration),
        Box::new(m20260821_000003_audit_logs::Migration),
        Box::new(m20260821_000004_push_subscriptions::Migration),
        Box::new(m20260922_000003_metrics_1m::Migration),
        Box::new(m20261006_000001_storage_destinations::Migration),
        Box::new(m20261008_000001_system_backup_plan::Migration),
        Box::new(m20261023_000001_scheduled_tasks::Migration),
    ]
}

/// Só o esquema do núcleo — o banco dos testes do `core` e do `nucleo`.
pub struct CoreMigrator;

impl MigratorTrait for CoreMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        migrations()
    }
}

/// Migrator dos testes de um crate de área: o esquema do núcleo mais o da área
/// (`netmonitor_core::area_test_migrator!(TestMigrator, migrations);`).
#[macro_export]
macro_rules! area_test_migrator {
    ($name:ident, $area:path) => {
        pub struct $name;

        impl $crate::migrations::MigratorTrait for $name {
            fn migrations() -> Vec<Box<dyn $crate::migrations::MigrationTrait>> {
                let mut all = $crate::migrations::migrations();
                all.extend($area());
                all
            }
        }
    };
}
