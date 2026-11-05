#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod shared;

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
mod m20260810_000013_discovery_runs;
mod m20260810_000014_discovery_results;
mod m20260810_000015_alert_rules;
mod m20260810_000016_alert_events;
mod m20260810_000017_vpn_servers;
mod m20260810_000018_vpn_peers;
mod m20260810_000019_dns_servers;
mod m20260810_000020_event_outbox;
mod m20260810_000021_probe_tasks;
mod m20260810_000022_system_settings;
mod m20260815_000003_notification_hygiene;
mod m20260821_000001_maintenance_windows;
mod m20260821_000002_monitor_results_hourly;
mod m20260821_000003_audit_logs;
mod m20260821_000004_push_subscriptions;
mod m20260922_000001_ai_conversations;
mod m20260922_000003_metrics_1m;
mod m20260930_000001_device_plugins;
mod m20261001_000001_device_plugin_installs;
mod m20261002_000001_plugin_fleet;
mod m20261005_000001_oui_vendors;
mod m20261006_000001_storage_destinations;
mod m20261007_000001_database_connections;
mod m20261008_000001_system_backup_plan;
mod m20261016_000001_link_tests;
mod m20261023_000001_scheduled_tasks;
mod m20261027_000001_wifi_watched_clients;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    /// Ordem de criação: uma tabela só nasce depois dos alvos das suas FKs. A
    /// mesma ordem, invertida, está em `src/models/tables.rs` para a limpeza
    /// entre testes — e um teste garante que as duas listas não divirjam.
    ///
    /// **Uma migration por tabela (ou grupo de tabelas), e só de criação.** O
    /// sistema nunca foi instalado: mudar o esquema é editar a migration que
    /// cria a tabela e recriar o banco de desenvolvimento — nada de migration
    /// de `ALTER`, de dados ou de compatibilidade (DB-02).
    ///
    /// **`auth_tokens` não é criada.** A autenticação é `loco_rs::auth::JWT`,
    /// stateless, e não guarda tokens de sessão no banco. O nome permanece no
    /// `CREATION_ORDER` para uma eventual adoção futura de tokens opacos.
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
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
            Box::new(m20260810_000013_discovery_runs::Migration),
            Box::new(m20260810_000014_discovery_results::Migration),
            Box::new(m20260810_000015_alert_rules::Migration),
            Box::new(m20260810_000016_alert_events::Migration),
            Box::new(m20260810_000017_vpn_servers::Migration),
            Box::new(m20260810_000018_vpn_peers::Migration),
            Box::new(m20260810_000019_dns_servers::Migration),
            Box::new(m20260810_000020_event_outbox::Migration),
            Box::new(m20260810_000021_probe_tasks::Migration),
            Box::new(m20260810_000022_system_settings::Migration),
            Box::new(m20260815_000003_notification_hygiene::Migration),
            Box::new(m20260821_000001_maintenance_windows::Migration),
            Box::new(m20260821_000002_monitor_results_hourly::Migration),
            Box::new(m20260821_000003_audit_logs::Migration),
            Box::new(m20260821_000004_push_subscriptions::Migration),
            Box::new(m20260922_000001_ai_conversations::Migration),
            Box::new(m20260922_000003_metrics_1m::Migration),
            Box::new(m20260930_000001_device_plugins::Migration),
            Box::new(m20261001_000001_device_plugin_installs::Migration),
            Box::new(m20261002_000001_plugin_fleet::Migration),
            Box::new(m20261005_000001_oui_vendors::Migration),
            Box::new(m20261006_000001_storage_destinations::Migration),
            Box::new(m20261007_000001_database_connections::Migration),
            Box::new(m20261008_000001_system_backup_plan::Migration),
            Box::new(m20261016_000001_link_tests::Migration),
            Box::new(m20261023_000001_scheduled_tasks::Migration),
            Box::new(m20261027_000001_wifi_watched_clients::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
