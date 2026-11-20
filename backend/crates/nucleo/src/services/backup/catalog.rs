//! O que entra em cada tipo de backup e como cada tabela é lida e gravada.
//!
//! Dois escopos com o **mesmo formato de arquivo** (o envelope de
//! [`super::service::BackupFile`]):
//!
//! * **configuração** ([`BackupScope::Config`]) — o grafo que o operador
//!   cadastrou ([`super::service::BACKED_UP_TABLES`]), sem histórico nem
//!   usuários; é o que o plano copia para os destinos;
//! * **completo** ([`BackupScope::Full`]) — todas as tabelas do banco
//!   principal, inclusive usuários e histórico, e as do banco de logs. É a
//!   cópia para recuperar a instalação inteira (ou levá-la a outro servidor).
//!
//! Fica fora do completo só o que se refaz sozinho ou não pode voltar:
//! filas de entrega (reenviariam notificações e eventos antigos), envios de
//! imagem pela metade e as bases públicas que o sistema baixa de novo.

use futures::{future::BoxFuture, StreamExt};
use sea_orm::{
    ActiveModelTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, Iterable,
    PrimaryKeyToColumn, QueryOrder,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use ts_rs::TS;

use super::archive::Chunker;
use crate::{
    models::{_entities, logs},
    services::shared::errors::{AppError, AppResult},
};

/// O que o arquivo de backup carrega.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum BackupScope {
    /// Só a configuração; usuários e histórico ficam como estão.
    #[default]
    Config,
    /// Os bancos inteiros: substitui também usuários, histórico e logs.
    Full,
}

/// O banco de onde a tabela vem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Store {
    Main,
    Logs,
}

/// Linhas cruas de uma tabela, no formato em que a entidade serializa.
pub type Rows = Vec<serde_json::Value>;

type DumpFn = for<'a> fn(&'a DatabaseTransaction, &'a mut Chunker) -> BoxFuture<'a, AppResult<u64>>;
type InsertFn =
    for<'a> fn(&'a DatabaseTransaction, &'static str, Rows) -> BoxFuture<'a, AppResult<()>>;

/// Uma tabela do backup completo: como despejá-la e como recarregá-la.
pub struct TableIo {
    pub name: &'static str,
    pub store: Store,
    dump: DumpFn,
    insert: InsertFn,
}

impl TableIo {
    /// Escreve as linhas da tabela no arquivo, em ordem de chave primária.
    pub async fn dump(&self, txn: &DatabaseTransaction, out: &mut Chunker) -> AppResult<u64> {
        (self.dump)(txn, out).await
    }

    /// Grava as linhas com os ids do arquivo.
    pub async fn insert(&self, txn: &DatabaseTransaction, rows: Rows) -> AppResult<()> {
        (self.insert)(txn, self.name, rows).await
    }
}

/// Tabelas do esquema que o backup completo não leva.
pub const NOT_IN_FULL_BACKUP: [&str; 8] = [
    // Bases públicas: o sistema as baixa de novo.
    "oui_vendors",
    "registry_advisories",
    "registry_feeds",
    // Filas de entrega: voltar com elas reenviaria o que já foi entregue.
    "event_outbox",
    "notification_outbox",
    "probe_tasks",
    // Envio de imagem pela metade: o arquivo dele não vai junto.
    "registry_uploads",
    // Não existe no esquema (a sessão é JWT, sem tabela).
    "auth_tokens",
];

/// Tabelas que a restauração completa **não** apaga: as bases públicas, que
/// valem para qualquer instalação e não citam nada do que é substituído.
pub const KEPT_ON_FULL_RESTORE: [&str; 3] =
    ["oui_vendors", "registry_advisories", "registry_feeds"];

macro_rules! table_io {
    ($store:expr, $module:path, $name:literal) => {
        TableIo {
            name: $name,
            store: $store,
            dump: {
                use $module as m;
                dump_boxed::<m::Entity>
            },
            insert: {
                use $module as m;
                insert_boxed::<m::Entity>
            },
        }
    };
}

/// As tabelas do backup completo, na ordem de criação (pai antes de filho):
/// é a ordem em que voltam sem violar FK.
static FULL: &[TableIo] = &[
    table_io!(Store::Main, _entities::users, "users"),
    table_io!(Store::Main, _entities::sites, "sites"),
    table_io!(Store::Main, _entities::probes, "probes"),
    table_io!(Store::Main, _entities::networks, "networks"),
    table_io!(Store::Main, _entities::devices, "devices"),
    table_io!(
        Store::Main,
        _entities::device_interfaces,
        "device_interfaces"
    ),
    table_io!(Store::Main, _entities::device_links, "device_links"),
    table_io!(Store::Main, _entities::monitors, "monitors"),
    table_io!(Store::Main, _entities::monitor_results, "monitor_results"),
    table_io!(
        Store::Main,
        _entities::monitor_results_hourly,
        "monitor_results_hourly"
    ),
    table_io!(Store::Main, _entities::metrics, "metrics"),
    table_io!(Store::Main, _entities::discovery_runs, "discovery_runs"),
    table_io!(
        Store::Main,
        _entities::discovery_results,
        "discovery_results"
    ),
    table_io!(Store::Main, _entities::alert_rules, "alert_rules"),
    table_io!(Store::Main, _entities::alert_events, "alert_events"),
    table_io!(Store::Main, _entities::vpn_servers, "vpn_servers"),
    table_io!(Store::Main, _entities::vpn_peers, "vpn_peers"),
    table_io!(Store::Main, _entities::dns_servers, "dns_servers"),
    table_io!(Store::Main, _entities::system_settings, "system_settings"),
    table_io!(
        Store::Main,
        _entities::maintenance_windows,
        "maintenance_windows"
    ),
    table_io!(Store::Main, _entities::audit_logs, "audit_logs"),
    table_io!(
        Store::Main,
        _entities::push_subscriptions,
        "push_subscriptions"
    ),
    table_io!(Store::Main, _entities::ai_conversations, "ai_conversations"),
    table_io!(Store::Main, _entities::host_metrics_1m, "host_metrics_1m"),
    table_io!(
        Store::Main,
        _entities::container_metrics_1m,
        "container_metrics_1m"
    ),
    table_io!(Store::Main, _entities::plugins, "plugins"),
    table_io!(
        Store::Main,
        _entities::device_credentials,
        "device_credentials"
    ),
    table_io!(Store::Main, _entities::plugin_runs, "plugin_runs"),
    table_io!(
        Store::Main,
        _entities::plugin_auto_accept,
        "plugin_auto_accept"
    ),
    table_io!(
        Store::Main,
        _entities::device_plugin_installs,
        "device_plugin_installs"
    ),
    table_io!(Store::Main, _entities::plugin_settings, "plugin_settings"),
    table_io!(Store::Main, _entities::plugin_batches, "plugin_batches"),
    table_io!(
        Store::Main,
        _entities::storage_destinations,
        "storage_destinations"
    ),
    table_io!(
        Store::Main,
        _entities::database_connections,
        "database_connections"
    ),
    table_io!(Store::Main, _entities::database_backups, "database_backups"),
    table_io!(
        Store::Main,
        _entities::deploy_credentials,
        "deploy_credentials"
    ),
    table_io!(
        Store::Main,
        _entities::deploy_github_connections,
        "deploy_github_connections"
    ),
    table_io!(
        Store::Main,
        _entities::deploy_applications,
        "deploy_applications"
    ),
    table_io!(Store::Main, _entities::deploy_targets, "deploy_targets"),
    table_io!(
        Store::Main,
        _entities::deploy_env_versions,
        "deploy_env_versions"
    ),
    table_io!(
        Store::Main,
        _entities::deploy_build_secrets,
        "deploy_build_secrets"
    ),
    table_io!(Store::Main, _entities::deploy_builds, "deploy_builds"),
    table_io!(Store::Main, _entities::deploy_runs, "deploy_runs"),
    table_io!(
        Store::Main,
        _entities::registry_repositories,
        "registry_repositories"
    ),
    table_io!(Store::Main, _entities::registry_blobs, "registry_blobs"),
    table_io!(
        Store::Main,
        _entities::registry_repository_blobs,
        "registry_repository_blobs"
    ),
    table_io!(
        Store::Main,
        _entities::registry_manifests,
        "registry_manifests"
    ),
    table_io!(Store::Main, _entities::registry_tags, "registry_tags"),
    table_io!(Store::Main, _entities::registry_tokens, "registry_tokens"),
    table_io!(
        Store::Main,
        _entities::registry_findings,
        "registry_findings"
    ),
    table_io!(
        Store::Main,
        _entities::registry_exceptions,
        "registry_exceptions"
    ),
    table_io!(
        Store::Main,
        _entities::system_backup_plan,
        "system_backup_plan"
    ),
    table_io!(
        Store::Main,
        _entities::link_test_schedules,
        "link_test_schedules"
    ),
    table_io!(Store::Main, _entities::link_tests, "link_tests"),
    table_io!(Store::Main, _entities::link_test_legs, "link_test_legs"),
    table_io!(Store::Main, _entities::scheduled_tasks, "scheduled_tasks"),
    table_io!(
        Store::Main,
        _entities::scheduled_task_runs,
        "scheduled_task_runs"
    ),
    table_io!(
        Store::Main,
        _entities::wifi_watched_clients,
        "wifi_watched_clients"
    ),
    table_io!(
        Store::Main,
        _entities::wifi_roaming_events,
        "wifi_roaming_events"
    ),
    table_io!(Store::Logs, logs::log_templates, "log_templates"),
    table_io!(Store::Logs, logs::device_logs, "device_logs"),
];

/// As tabelas do backup completo de um banco, na ordem de criação.
pub fn full_tables(store: Store) -> impl Iterator<Item = &'static TableIo> {
    FULL.iter().filter(move |table| table.store == store)
}

/// A tabela do backup completo com este nome, se houver.
#[must_use]
pub fn find(store: Store, name: &str) -> Option<&'static TableIo> {
    full_tables(store).find(|table| table.name == name)
}

/// A restauração deste escopo grava esta tabela? Tabela desconhecida (de uma
/// versão mais nova, por exemplo) é ignorada, não recusada.
#[must_use]
pub fn accepts(scope: BackupScope, store: Store, table: &str) -> bool {
    match scope {
        BackupScope::Config => {
            store == Store::Main && super::service::BACKED_UP_TABLES.contains(&table)
        }
        BackupScope::Full => find(store, table).is_some(),
    }
}

fn dump_boxed<'a, E>(
    txn: &'a DatabaseTransaction,
    out: &'a mut Chunker,
) -> BoxFuture<'a, AppResult<u64>>
where
    E: EntityTrait,
    E::Model: Serialize + Send + Sync,
{
    Box::pin(dump_rows::<E>(txn, out))
}

/// Lê a tabela em fluxo — nunca inteira na memória — e escreve cada linha.
async fn dump_rows<E>(txn: &DatabaseTransaction, out: &mut Chunker) -> AppResult<u64>
where
    E: EntityTrait,
    E::Model: Serialize + Send + Sync,
{
    let mut query = E::find();
    for key in E::PrimaryKey::iter() {
        query = query.order_by_asc(key.into_column());
    }
    let mut rows = query.stream(txn).await?;
    let mut written = 0;
    while let Some(row) = rows.next().await {
        out.element(&row?)?;
        out.flush_if_full().await?;
        written += 1;
    }
    Ok(written)
}

fn insert_boxed<'a, E>(
    txn: &'a DatabaseTransaction,
    table: &'static str,
    rows: Rows,
) -> BoxFuture<'a, AppResult<()>>
where
    E: EntityTrait,
    E::Model: DeserializeOwned + IntoActiveModel<E::ActiveModel>,
    E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
{
    Box::pin(insert_rows::<E>(txn, table, rows))
}

/// Grava as linhas em lotes, com os ids do arquivo.
///
/// O lote respeita o teto de parâmetros por comando (32.766 no SQLite,
/// 65.535 no PostgreSQL): linhas × colunas fica abaixo dos dois.
async fn insert_rows<E>(txn: &DatabaseTransaction, table: &'static str, rows: Rows) -> AppResult<()>
where
    E: EntityTrait,
    E::Model: DeserializeOwned + IntoActiveModel<E::ActiveModel>,
    E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
{
    let columns = E::Column::iter().count().max(1);
    let batch = (30_000 / columns).clamp(1, 500);
    let mut models = Vec::with_capacity(rows.len());
    for value in rows {
        let model: E::Model = serde_json::from_value(value).map_err(|err| {
            AppError::validation(format!(
                "A tabela '{table}' do backup não casa com o esquema atual: {err}"
            ))
        })?;
        // `reset_all` troca todo campo de `Unchanged` para `Set`: sem isso o
        // `insert` sairia sem colunas, e é também o que preserva o `id`.
        models.push(model.into_active_model().reset_all());
    }
    let mut pending = models.into_iter().peekable();
    while pending.peek().is_some() {
        let chunk: Vec<E::ActiveModel> = pending.by_ref().take(batch).collect();
        E::insert_many(chunk).exec_without_returning(txn).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::tables::{CREATION_ORDER, LOG_TABLES};

    #[test]
    fn o_completo_leva_todo_o_esquema_menos_as_excecoes_e_na_ordem() {
        let esperado: Vec<&str> = CREATION_ORDER
            .iter()
            .copied()
            .filter(|table| !NOT_IN_FULL_BACKUP.contains(table))
            .collect();
        let principal: Vec<&str> = full_tables(Store::Main).map(|t| t.name).collect();
        assert_eq!(principal, esperado);
        let mut logs: Vec<&str> = full_tables(Store::Logs).map(|t| t.name).collect();
        logs.sort_unstable();
        let mut esperado_logs = LOG_TABLES.to_vec();
        esperado_logs.sort_unstable();
        assert_eq!(logs, esperado_logs);
    }

    #[test]
    fn excecoes_e_tabelas_preservadas_sao_do_esquema() {
        for table in NOT_IN_FULL_BACKUP.iter().chain(KEPT_ON_FULL_RESTORE.iter()) {
            assert!(CREATION_ORDER.contains(table), "{table} fora do esquema");
        }
        for table in KEPT_ON_FULL_RESTORE {
            assert!(NOT_IN_FULL_BACKUP.contains(&table));
        }
    }

    #[test]
    fn cada_escopo_aceita_o_que_e_dele() {
        assert!(accepts(BackupScope::Config, Store::Main, "devices"));
        assert!(!accepts(BackupScope::Config, Store::Main, "users"));
        assert!(!accepts(BackupScope::Config, Store::Logs, "device_logs"));
        assert!(accepts(BackupScope::Full, Store::Main, "users"));
        assert!(accepts(BackupScope::Full, Store::Logs, "device_logs"));
        assert!(!accepts(
            BackupScope::Full,
            Store::Main,
            "notification_outbox"
        ));
        assert!(!accepts(BackupScope::Full, Store::Main, "tabela_do_futuro"));
    }
}
