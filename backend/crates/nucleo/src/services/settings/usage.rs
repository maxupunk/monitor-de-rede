//! O que ocupa o espaço do banco: tabela por tabela, do maior para o menor.
//!
//! Dois dialetos, duas fontes:
//!
//! * **PostgreSQL** — o catálogo responde sem varrer nada:
//!   `pg_table_size` (dados, com TOAST), `pg_indexes_size` e
//!   `pg_class.reltuples` (linhas estimadas pelo último `ANALYZE`). Tabela que
//!   nunca passou por `ANALYZE` (`reltuples = -1`) é contada de verdade.
//! * **SQLite** — o tamanho por tabela vem da tabela virtual `dbstat`, que só
//!   existe quando o SQLite foi compilado com `SQLITE_ENABLE_DBSTAT_VTAB`.
//!   Sem ela, a tela recebe a contagem exata de linhas e o tamanho do arquivo,
//!   e o aviso de que o detalhe por tabela não está disponível.
//!
//! O banco de logs (ADR 020) entra como uma seção à parte quando mora em outro
//! banco; no Postgres sem `SYSLOG_DB_URL` as tabelas dele já aparecem no
//! principal.

use std::collections::HashMap;

use netmonitor_core::areas::{Area, AreaPolicies};
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement};
use serde::Serialize;
use ts_rs::TS;

use super::database::DbType;
use crate::{
    models::tables,
    services::{
        maintenance::data_pruner,
        shared::errors::{AppError, AppResult},
        syslog::retention::LOGS_RETENTION,
    },
};

/// Qual banco a seção descreve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DatabaseKind {
    Main,
    Logs,
}

/// Uma tabela e o que ela ocupa.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TableUsage {
    /// Nome técnico.
    pub table: String,
    /// Nome em português.
    pub label: String,
    /// Área do produto dona da tabela (ADR 017).
    pub area: Area,
    pub area_title: String,
    /// Dados + índices. `None` quando o banco não informa (SQLite sem `dbstat`).
    #[ts(type = "number | null")]
    pub total_bytes: Option<i64>,
    #[ts(type = "number | null")]
    pub data_bytes: Option<i64>,
    #[ts(type = "number | null")]
    pub index_bytes: Option<i64>,
    #[ts(type = "number")]
    pub rows: i64,
    /// A contagem é a estimativa do catálogo, não um `COUNT(*)`.
    pub rows_estimated: bool,
    /// Quantos dias a purga automática guarda; `None` quando a tabela não
    /// envelhece sozinha.
    #[ts(type = "number | null")]
    pub retention_days: Option<i64>,
}

/// Um banco: o tamanho total e as tabelas dele.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseUsageSection {
    pub kind: DatabaseKind,
    pub title: String,
    /// O banco no servidor (Postgres) ou o arquivo (SQLite).
    pub name: Option<String>,
    #[ts(type = "number")]
    pub size_bytes: i64,
    /// O que o total tem além das tabelas listadas: catálogo, sequências,
    /// páginas livres. `None` sem o detalhe por tabela.
    #[ts(type = "number | null")]
    pub other_bytes: Option<i64>,
    /// O banco informa o tamanho de cada tabela.
    pub detail_available: bool,
    /// Do maior para o menor.
    pub tables: Vec<TableUsage>,
}

/// Resposta de `GET /api/settings/database-usage`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseUsage {
    pub db_type: DbType,
    #[ts(type = "number")]
    pub total_bytes: i64,
    pub databases: Vec<DatabaseUsageSection>,
}

/// Uma linha crua do catálogo, antes do nome legível e da área.
#[derive(Debug, Clone, PartialEq)]
struct RawTable {
    table: String,
    data_bytes: Option<i64>,
    index_bytes: Option<i64>,
    rows: i64,
    rows_estimated: bool,
}

/// O que ocupa o espaço do banco principal e, quando separado, do de logs.
///
/// # Errors
///
/// Propaga falha de consulta ao catálogo do banco principal. O banco de logs
/// fora do ar não derruba a resposta: a seção dele só some.
pub async fn database_usage<C: ConnectionTrait, L: ConnectionTrait>(
    db: &C,
    logs: Option<&L>,
    policies: &AreaPolicies,
) -> AppResult<DatabaseUsage> {
    let db_type = match db.get_database_backend() {
        DatabaseBackend::Postgres => DbType::Postgres,
        _ => DbType::Sqlite,
    };
    let mut databases = vec![section(db, DatabaseKind::Main, policies).await?];
    if let Some(logs) = logs {
        match section(logs, DatabaseKind::Logs, policies).await {
            Ok(logs_section) => databases.push(logs_section),
            Err(error) => tracing::warn!(%error, "não foi possível medir o banco de logs"),
        }
    }
    Ok(DatabaseUsage {
        db_type,
        total_bytes: databases.iter().map(|s| s.size_bytes).sum(),
        databases,
    })
}

async fn section<C: ConnectionTrait>(
    db: &C,
    kind: DatabaseKind,
    policies: &AreaPolicies,
) -> AppResult<DatabaseUsageSection> {
    let (name, size_bytes, raw, detail_available) = match db.get_database_backend() {
        DatabaseBackend::Postgres => {
            let name = scalar_string(db, "SELECT current_database()::text AS v").await?;
            let size =
                scalar_i64(db, "SELECT pg_database_size(current_database())::int8 AS v").await?;
            (Some(name), size, postgres_tables(db).await?, true)
        }
        _ => {
            let size = sqlite_file_bytes(db).await?;
            let name = sqlite_file_name(db).await;
            match sqlite_tables_with_dbstat(db).await {
                Ok(raw) => (name, size, raw, true),
                Err(error) => {
                    tracing::debug!(%error, "dbstat indisponível; só a contagem de linhas");
                    (name, size, sqlite_tables_counted(db).await?, false)
                }
            }
        }
    };

    let mut tables: Vec<TableUsage> = raw
        .into_iter()
        .map(|raw| describe(raw, kind, policies))
        .collect();
    sort_largest_first(&mut tables);

    let other_bytes = detail_available.then(|| {
        let listed: i64 = tables.iter().filter_map(|t| t.total_bytes).sum();
        (size_bytes - listed).max(0)
    });

    Ok(DatabaseUsageSection {
        kind,
        title: match kind {
            DatabaseKind::Main => "Banco principal".to_owned(),
            DatabaseKind::Logs => "Banco de logs (Syslog)".to_owned(),
        },
        name,
        size_bytes,
        other_bytes,
        detail_available,
        tables,
    })
}

fn describe(raw: RawTable, kind: DatabaseKind, policies: &AreaPolicies) -> TableUsage {
    let area = policies.area_of_table(&raw.table);
    let total_bytes = match (raw.data_bytes, raw.index_bytes) {
        (Some(data), Some(index)) => Some(data + index),
        (Some(data), None) => Some(data),
        _ => None,
    };
    TableUsage {
        label: tables::label(&raw.table).to_owned(),
        area,
        area_title: area.title().to_owned(),
        total_bytes,
        data_bytes: raw.data_bytes,
        index_bytes: raw.index_bytes,
        rows: raw.rows,
        rows_estimated: raw.rows_estimated,
        retention_days: retention_days_of(&raw.table, kind),
        table: raw.table,
    }
}

/// A janela da purga automática, quando a tabela envelhece sozinha.
fn retention_days_of(table: &str, kind: DatabaseKind) -> Option<i64> {
    if table == "device_logs" {
        return Some(LOGS_RETENTION.days());
    }
    match kind {
        DatabaseKind::Main => {
            data_pruner::retention_of(table).map(data_pruner::RetentionRule::days)
        }
        DatabaseKind::Logs => None,
    }
}

/// Do maior para o menor; sem tamanho (SQLite sem `dbstat`), pelas linhas.
fn sort_largest_first(tables: &mut [TableUsage]) {
    tables.sort_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then(b.rows.cmp(&a.rows))
            .then_with(|| a.table.cmp(&b.table))
    });
}

async fn postgres_tables<C: ConnectionTrait>(db: &C) -> AppResult<Vec<RawTable>> {
    let rows = db
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT c.relname::text AS name, \
                    pg_table_size(c.oid)::int8 AS data_bytes, \
                    pg_indexes_size(c.oid)::int8 AS index_bytes, \
                    c.reltuples::float8 AS estimated_rows \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r', 'p') AND n.nspname = current_schema()",
        ))
        .await?;
    let mut tables = Vec::with_capacity(rows.len());
    for row in rows {
        let table: String = get(&row, "name")?;
        let estimated: f64 = get(&row, "estimated_rows")?;
        // `-1`: a tabela nunca passou por `ANALYZE` — em geral é nova e pequena.
        let (rows, rows_estimated) = if estimated < 0.0 {
            (count_rows(db, &table).await?, false)
        } else {
            (rounded(estimated), true)
        };
        tables.push(RawTable {
            data_bytes: Some(get(&row, "data_bytes")?),
            index_bytes: Some(get(&row, "index_bytes")?),
            table,
            rows,
            rows_estimated,
        });
    }
    Ok(tables)
}

/// O tamanho de cada tabela e dos índices dela pela `dbstat`.
async fn sqlite_tables_with_dbstat<C: ConnectionTrait>(db: &C) -> AppResult<Vec<RawTable>> {
    let pages = db
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name, SUM(pgsize) AS bytes FROM dbstat GROUP BY name",
        ))
        .await?;
    let mut bytes_by_object = HashMap::with_capacity(pages.len());
    for row in pages {
        bytes_by_object.insert(get::<String>(&row, "name")?, get::<i64>(&row, "bytes")?);
    }

    let objects = db
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name, tbl_name, type FROM sqlite_master \
             WHERE type IN ('table', 'index') AND name NOT LIKE 'sqlite_%'",
        ))
        .await?;
    let mut data: HashMap<String, i64> = HashMap::new();
    let mut index: HashMap<String, i64> = HashMap::new();
    for row in objects {
        let name: String = get(&row, "name")?;
        let owner: String = get(&row, "tbl_name")?;
        let kind: String = get(&row, "type")?;
        let bytes = bytes_by_object.get(&name).copied().unwrap_or(0);
        if kind == "table" {
            *data.entry(owner).or_default() += bytes;
        } else {
            *index.entry(owner).or_default() += bytes;
        }
    }
    // Índice automático (`sqlite_autoindex_*`) é de uma tabela, mas o nome
    // dele fica fora do filtro acima: soma pelo dono que o `dbstat` não dá.
    for (name, bytes) in &bytes_by_object {
        if let Some(owner) = autoindex_owner(name) {
            if data.contains_key(owner) {
                *index.entry(owner.to_owned()).or_default() += bytes;
            }
        }
    }

    let mut tables = Vec::with_capacity(data.len());
    for (table, data_bytes) in data {
        tables.push(RawTable {
            rows: count_rows(db, &table).await?,
            rows_estimated: false,
            index_bytes: Some(index.get(&table).copied().unwrap_or(0)),
            data_bytes: Some(data_bytes),
            table,
        });
    }
    Ok(tables)
}

/// `sqlite_autoindex_users_1` → `users`.
fn autoindex_owner(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("sqlite_autoindex_")?;
    rest.rsplit_once('_').map(|(owner, _)| owner)
}

/// Sem `dbstat`: só a contagem de linhas de cada tabela.
async fn sqlite_tables_counted<C: ConnectionTrait>(db: &C) -> AppResult<Vec<RawTable>> {
    let names = db
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        ))
        .await?;
    let mut tables = Vec::with_capacity(names.len());
    for row in names {
        let table: String = get(&row, "name")?;
        tables.push(RawTable {
            rows: count_rows(db, &table).await?,
            rows_estimated: false,
            data_bytes: None,
            index_bytes: None,
            table,
        });
    }
    Ok(tables)
}

async fn count_rows<C: ConnectionTrait>(db: &C, table: &str) -> AppResult<i64> {
    // O nome vem do catálogo do próprio banco, nunca de quem chamou; as aspas
    // duplas cobrem nome com caractere especial.
    let quoted = table.replace('"', "\"\"");
    scalar_i64(db, &format!("SELECT COUNT(*) AS v FROM \"{quoted}\"")).await
}

async fn sqlite_file_bytes<C: ConnectionTrait>(db: &C) -> AppResult<i64> {
    let pages = scalar_i64(db, "SELECT page_count AS v FROM pragma_page_count()").await?;
    let size = scalar_i64(db, "SELECT page_size AS v FROM pragma_page_size()").await?;
    pages
        .checked_mul(size)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("tamanho do SQLite excede i64")))
}

/// O nome do arquivo do banco, sem o caminho; `None` no banco em memória.
async fn sqlite_file_name<C: ConnectionTrait>(db: &C) -> Option<String> {
    let path = scalar_string(
        db,
        "SELECT file AS v FROM pragma_database_list() WHERE name = 'main'",
    )
    .await
    .ok()?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
    (!name.is_empty()).then(|| name.to_owned())
}

async fn scalar_i64<C: ConnectionTrait>(db: &C, sql: &str) -> AppResult<i64> {
    let row = one(db, sql).await?;
    get(&row, "v")
}

async fn scalar_string<C: ConnectionTrait>(db: &C, sql: &str) -> AppResult<String> {
    let row = one(db, sql).await?;
    get(&row, "v")
}

async fn one<C: ConnectionTrait>(db: &C, sql: &str) -> AppResult<QueryResult> {
    db.query_one_raw(Statement::from_string(
        db.get_database_backend(),
        sql.to_owned(),
    ))
    .await?
    .ok_or_else(|| AppError::Internal(anyhow::anyhow!("consulta sem linha: {sql}")))
}

fn get<T: sea_orm::TryGetable>(row: &QueryResult, column: &str) -> AppResult<T> {
    row.try_get("", column)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))
}

#[allow(clippy::cast_possible_truncation)]
fn rounded(value: f64) -> i64 {
    value.round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use netmonitor_core::migrations::{CoreMigrator as Migrator, MigratorTrait};
    use sea_orm::DatabaseConnection;

    async fn sqlite_memory() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    fn raw(table: &str, bytes: Option<i64>, rows: i64) -> TableUsage {
        describe(
            RawTable {
                table: table.to_owned(),
                data_bytes: bytes,
                index_bytes: bytes.map(|_| 0),
                rows,
                rows_estimated: false,
            },
            DatabaseKind::Main,
            &AreaPolicies::default(),
        )
    }

    #[test]
    fn ordena_do_maior_para_o_menor_e_sem_tamanho_pelas_linhas() {
        let mut com_tamanho = vec![
            raw("sites", Some(10), 1),
            raw("metrics", Some(900), 5),
            raw("devices", Some(50), 3),
        ];
        sort_largest_first(&mut com_tamanho);
        let ordem: Vec<&str> = com_tamanho.iter().map(|t| t.table.as_str()).collect();
        assert_eq!(ordem, ["metrics", "devices", "sites"]);

        let mut sem_tamanho = vec![raw("sites", None, 1), raw("metrics", None, 5)];
        sort_largest_first(&mut sem_tamanho);
        assert_eq!(sem_tamanho[0].table, "metrics");
        assert_eq!(sem_tamanho[0].total_bytes, None);
    }

    #[test]
    fn nome_legivel_area_e_retencao() {
        let metricas = raw("metrics", Some(1), 0);
        assert_eq!(metricas.label, "Métricas dos dispositivos");
        assert_eq!(metricas.area, Area::Core);
        assert_eq!(metricas.retention_days, Some(30));
        assert_eq!(raw("devices", Some(1), 0).retention_days, None);
        assert_eq!(
            retention_days_of("device_logs", DatabaseKind::Logs),
            Some(LOGS_RETENTION.days())
        );
    }

    #[test]
    fn dono_do_indice_automatico() {
        assert_eq!(autoindex_owner("sqlite_autoindex_users_1"), Some("users"));
        assert_eq!(
            autoindex_owner("sqlite_autoindex_device_logs_2"),
            Some("device_logs")
        );
        assert_eq!(autoindex_owner("idx_users_email"), None);
    }

    #[tokio::test]
    async fn sqlite_lista_as_tabelas_com_linhas() {
        let db = sqlite_memory().await;
        db.execute_unprepared(
            "INSERT INTO sites (name, active, created_at, updated_at) \
             VALUES ('A', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'), \
                    ('B', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .await
        .expect("sites");

        let usage = database_usage::<_, DatabaseConnection>(&db, None, &AreaPolicies::default())
            .await
            .expect("uso");
        assert_eq!(usage.db_type, DbType::Sqlite);
        assert_eq!(usage.databases.len(), 1);
        let main = &usage.databases[0];
        assert_eq!(main.kind, DatabaseKind::Main);
        assert!(main.size_bytes > 0);
        assert_eq!(usage.total_bytes, main.size_bytes);
        let sites = main
            .tables
            .iter()
            .find(|t| t.table == "sites")
            .expect("sites listada");
        assert_eq!(sites.rows, 2);
        assert_eq!(sites.label, "Sites");
        assert!(!sites.rows_estimated);
        // Com ou sem `dbstat`, a resposta é coerente com o que ela diz.
        if main.detail_available {
            assert!(sites.total_bytes.is_some());
            assert!(main.other_bytes.is_some());
        } else {
            assert!(main.tables.iter().all(|t| t.total_bytes.is_none()));
        }
    }

    #[tokio::test]
    async fn sqlite_sem_dbstat_conta_as_linhas_sem_tamanho() {
        let db = sqlite_memory().await;
        let tabelas = sqlite_tables_counted(&db).await.expect("contagem");
        let users = tabelas
            .iter()
            .find(|t| t.table == "users")
            .expect("users listada");
        assert_eq!(users.rows, 0);
        assert_eq!(users.data_bytes, None);
        assert!(tabelas.iter().all(|t| !t.table.starts_with("sqlite_")));
    }

    #[tokio::test]
    async fn banco_de_logs_vira_secao_propria() {
        let db = sqlite_memory().await;
        let logs = crate::services::syslog::db::connect("sqlite::memory:")
            .await
            .expect("logs");
        let usage = database_usage(&db, Some(logs.connection()), &AreaPolicies::default())
            .await
            .expect("uso");
        assert_eq!(usage.databases.len(), 2);
        let secao = &usage.databases[1];
        assert_eq!(secao.kind, DatabaseKind::Logs);
        assert!(secao.tables.iter().any(|t| t.table == "device_logs"));
        assert_eq!(
            usage.total_bytes,
            usage.databases[0].size_bytes + secao.size_bytes
        );
    }
}
