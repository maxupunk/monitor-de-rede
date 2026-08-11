#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod shared;

mod m20220101_000001_users;
mod m20260810_000001_users_active;
mod m20260810_000002_sites;
mod m20260810_000003_probes;
mod m20260810_000004_networks;
mod m20260810_000005_zabbix_templates;
mod m20260810_000006_zabbix_template_items;
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

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    /// Ordem de criação da §6: uma tabela só nasce depois dos alvos das suas
    /// FKs. A mesma ordem, invertida, está em `src/models/tables.rs` para a
    /// limpeza entre testes — e um teste garante que as duas listas não
    /// divirjam.
    ///
    /// **`auth_tokens` (§6 #23) não é criada.** A §10.2 decidiu por
    /// `loco_rs::auth::JWT`, que não guarda token no banco. A tabela
    /// `auth_access_tokens` do AdonisJS existia por causa do `@adonisjs/auth`;
    /// sem ela, são 22 tabelas. O nome segue listado em `CREATION_ORDER` para o
    /// dia em que a Fase 6 optar por tokens opacos.
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20260810_000001_users_active::Migration),
            Box::new(m20260810_000002_sites::Migration),
            Box::new(m20260810_000003_probes::Migration),
            Box::new(m20260810_000004_networks::Migration),
            Box::new(m20260810_000005_zabbix_templates::Migration),
            Box::new(m20260810_000006_zabbix_template_items::Migration),
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
            // inject-above (do not remove this comment)
        ]
    }
}
