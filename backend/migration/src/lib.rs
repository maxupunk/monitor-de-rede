#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod shared;

/// Migrator do banco de logs (`/data/logs.sqlite`), separado do principal.
pub mod logs;

mod m20220101_000001_users;
mod m20260810_000001_users_active;
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
mod m20260812_000001_drop_zabbix_templates;
mod m20260814_000001_device_snmp_poll_interval;
mod m20260815_000001_alert_rules_recovery_window;
mod m20260815_000002_alert_rules_flap_detection;
mod m20260815_000003_notification_hygiene;
mod m20260816_000002_device_access_mode;
mod m20260817_000001_device_operating_system;
mod m20260818_000001_users_role;
mod m20260819_000001_devices_system_key;
mod m20260819_000002_monitors_managed_unique;
mod m20260821_000001_maintenance_windows;
mod m20260821_000002_monitor_results_hourly;
mod m20260821_000003_audit_logs;
mod m20260821_000004_push_subscriptions;
mod m20260822_000001_icmp_filtered_alert;

pub struct Migrator;

/// Migrations que existiram, foram aplicadas em bancos reais e depois saíram do
/// repositório.
///
/// O `sea-orm-migration` aborta o `up()` quando encontra em `seaql_migrations`
/// uma versão sem arquivo correspondente ("migration file of version … is
/// missing"). Como o esquema de templates Zabbix foi removido inteiro, os
/// bancos que já rodaram aquelas duas migrations precisam ter o registro
/// apagado **antes** do migrator rodar — é o que
/// [`crate::purge_removed_migrations`] faz, chamado no `after_context` do
/// `App`.
pub const REMOVED_MIGRATIONS: [&str; 2] = [
    "m20260810_000005_zabbix_templates",
    "m20260810_000006_zabbix_template_items",
];

/// Apaga de `seaql_migrations` os registros das migrations em
/// [`REMOVED_MIGRATIONS`].
///
/// Roda antes do migrator e é idempotente: num banco novo a tabela ainda não
/// existe e a função não faz nada.
///
/// # Errors
///
/// Propaga falha do banco ao consultar ou limpar `seaql_migrations`.
pub async fn purge_removed_migrations<C: sea_orm::ConnectionTrait>(
    db: &C,
) -> Result<(), sea_orm::DbErr> {
    use sea_orm::{DatabaseBackend, Statement};

    let backend = db.get_database_backend();
    let exists = match backend {
        DatabaseBackend::Sqlite => {
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'seaql_migrations'"
        }
        _ => {
            "SELECT table_name FROM information_schema.tables \
              WHERE table_schema = current_schema() AND table_name = 'seaql_migrations'"
        }
    };
    if db
        .query_all_raw(Statement::from_string(backend, exists))
        .await?
        .is_empty()
    {
        return Ok(());
    }

    let list = REMOVED_MIGRATIONS
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ");
    db.execute_raw(Statement::from_string(
        backend,
        format!("DELETE FROM seaql_migrations WHERE version IN ({list})"),
    ))
    .await?;
    Ok(())
}

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    /// Ordem de criação: uma tabela só nasce depois dos alvos das suas FKs. A
    /// mesma ordem, invertida, está em `src/models/tables.rs` para a limpeza
    /// entre testes — e um teste garante que as duas listas não divirjam.
    ///
    /// **`auth_tokens` não é criada.** A autenticação é `loco_rs::auth::JWT`,
    /// que é stateless e não guarda token no banco. A tabela
    /// `auth_access_tokens` que existia no esquema anterior vinha do
    /// `@adonisjs/auth`, e sumiu junto com ele. O nome segue listado em
    /// `CREATION_ORDER` para o dia em que a escolha for por tokens opacos.
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20260810_000001_users_active::Migration),
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
            Box::new(m20260812_000001_drop_zabbix_templates::Migration),
            Box::new(m20260814_000001_device_snmp_poll_interval::Migration),
            Box::new(m20260815_000001_alert_rules_recovery_window::Migration),
            Box::new(m20260815_000002_alert_rules_flap_detection::Migration),
            Box::new(m20260815_000003_notification_hygiene::Migration),
            Box::new(m20260816_000002_device_access_mode::Migration),
            Box::new(m20260817_000001_device_operating_system::Migration),
            Box::new(m20260818_000001_users_role::Migration),
            Box::new(m20260819_000001_devices_system_key::Migration),
            Box::new(m20260819_000002_monitors_managed_unique::Migration),
            Box::new(m20260821_000001_maintenance_windows::Migration),
            Box::new(m20260821_000002_monitor_results_hourly::Migration),
            Box::new(m20260821_000003_audit_logs::Migration),
            Box::new(m20260821_000004_push_subscriptions::Migration),
            Box::new(m20260822_000001_icmp_filtered_alert::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
