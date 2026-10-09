//! PostgreSQL: dump pelo catálogo e `COPY`, restauração numa transação.
//!
//! ## A ordem do arquivo é a ordem que o restore precisa
//!
//! Esquemas → extensões → tipos → sequências → funções → tabelas (sem
//! restrições) → **dados** → valores das sequências → chaves e checks →
//! índices → chaves estrangeiras → views → triggers. Criar índices e FKs só
//! depois dos dados é o que o `pg_dump` faz e pelo mesmo motivo: carregar
//! numa tabela sem índice é ordens de grandeza mais rápido, e a FK não precisa
//! de ordem entre as tabelas.
//!
//! A definição de cada objeto vem do próprio servidor (`pg_get_*def`,
//! `format_type`, `pg_get_expr`), e os nomes saem citados por `format('%I')` —
//! nada de SQL é reconstruído à mão aqui.
//!
//! ## Foto consistente
//!
//! O dump inteiro roda numa transação `REPEATABLE READ READ ONLY`: o banco
//! continua recebendo escrita, mas o arquivo é uma foto de um instante só.
//!
//! ## O que fica de fora (e vira aviso)
//!
//! Dono e permissões (como `--no-owner --no-acl`), agregados, tipos compostos
//! e de intervalo, regras, tabelas estrangeiras e objetos grandes. Cada um que
//! existir no banco vira um aviso no histórico do backup.

use async_trait::async_trait;
use futures::TryStreamExt;
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    AssertSqlSafe, Connection, PgConnection, Row,
};

use super::{
    connect_with_timeout,
    dump::{DumpStats, DumpWriter},
    restore::{apply_script, LineReader, RestoreSession, RestoreStats},
    sql::{order::dependency_order, quote_ident, quote_literal, validate_database_name},
    DatabaseEngine, DatabaseError, DatabaseTarget, Driver, EngineFamily, Probe, Progress,
    RestoreMode, SslMode,
};

const FAMILY: EngineFamily = EngineFamily::Postgres;

/// Banco de manutenção, para listar e criar bancos.
const MAINTENANCE_DATABASE: &str = "postgres";

/// Versão mínima: `prokind` (11), `attgenerated` (12).
const MIN_VERSION_NUM: i32 = 120_000;

/// Esquemas do usuário (o alias do `pg_namespace` é `n`).
const USER_NAMESPACES: &str = "n.nspname NOT IN ('pg_catalog', 'information_schema') \
                               AND n.nspname NOT LIKE 'pg\\_%'";

/// Objeto que não pertence a uma extensão (a extensão o recria).
fn not_from_extension(catalog: &str, oid: &str) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM pg_depend dx WHERE dx.classid = '{catalog}'::regclass \
         AND dx.objid = {oid} AND dx.deptype = 'e')"
    )
}

pub struct PostgresDriver;

async fn connect(target: &DatabaseTarget, database: &str) -> Result<PgConnection, DatabaseError> {
    let options = PgConnectOptions::new()
        .host(&target.host)
        .port(target.port)
        .username(&target.username)
        .password(&target.password)
        .database(database)
        .application_name("netmonitor-backup")
        .ssl_mode(match target.ssl_mode {
            SslMode::Disable => PgSslMode::Disable,
            SslMode::Prefer => PgSslMode::Prefer,
            SslMode::Require => PgSslMode::Require,
        });
    connect_with_timeout(target, PgConnection::connect_with(&options)).await
}

fn maintenance(target: &DatabaseTarget) -> &str {
    target.database.as_deref().unwrap_or(MAINTENANCE_DATABASE)
}

async fn exec(conn: &mut PgConnection, sql: &str) -> Result<u64, DatabaseError> {
    sqlx::raw_sql(AssertSqlSafe(sql.to_string()))
        .execute(conn)
        .await
        .map(|done| done.rows_affected())
        .map_err(|error| DatabaseError::query(&error))
}

async fn rows(
    conn: &mut PgConnection,
    sql: &str,
) -> Result<Vec<sqlx::postgres::PgRow>, DatabaseError> {
    sqlx::query(AssertSqlSafe(sql.to_string()))
        .fetch_all(conn)
        .await
        .map_err(|error| DatabaseError::query(&error))
}

fn get<'r, T: sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>>(
    row: &'r sqlx::postgres::PgRow,
    column: &str,
) -> Result<T, DatabaseError> {
    row.try_get(column)
        .map_err(|error| DatabaseError::Query(format!("coluna {column}: {error}")))
}

async fn database_exists(conn: &mut PgConnection, name: &str) -> Result<bool, DatabaseError> {
    sqlx::query("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(name)
        .fetch_optional(conn)
        .await
        .map(|row| row.is_some())
        .map_err(|error| DatabaseError::query(&error))
}

#[async_trait]
impl Driver for PostgresDriver {
    async fn probe(&self, target: &DatabaseTarget) -> Result<Probe, DatabaseError> {
        let started = std::time::Instant::now();
        let mut conn = connect(target, maintenance(target)).await?;
        let version: String = sqlx::query_scalar("SHOW server_version")
            .fetch_one(&mut conn)
            .await
            .map_err(|error| DatabaseError::query(&error))?;
        let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
        let databases: Vec<String> = sqlx::query_scalar(
            "SELECT datname FROM pg_database \
             WHERE NOT datistemplate AND datallowconn \
             AND has_database_privilege(datname, 'CONNECT') ORDER BY datname",
        )
        .fetch_all(&mut conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        let _ = conn.close().await;
        Ok(Probe {
            latency_ms,
            version: format!("PostgreSQL {version}"),
            databases,
        })
    }

    async fn dump(
        &self,
        target: &DatabaseTarget,
        database: &str,
        out: &mut DumpWriter,
        progress: &dyn Progress,
    ) -> Result<DumpStats, DatabaseError> {
        let mut conn = connect(target, database).await?;
        let version_num: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::int")
                .fetch_one(&mut conn)
                .await
                .map_err(|error| DatabaseError::query(&error))?;
        if version_num < MIN_VERSION_NUM {
            return Err(DatabaseError::Unsupported(
                "O backup nativo pede PostgreSQL 12 ou mais novo".to_string(),
            ));
        }
        let version: String = sqlx::query_scalar("SHOW server_version")
            .fetch_one(&mut conn)
            .await
            .map_err(|error| DatabaseError::query(&error))?;

        exec(
            &mut conn,
            "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; \
             SET LOCAL statement_timeout = 0; SET LOCAL extra_float_digits = 3; \
             SET LOCAL DateStyle = ISO; SET LOCAL IntervalStyle = postgres",
        )
        .await?;

        let mut dumper = Dumper {
            conn: &mut conn,
            out,
            progress,
            stats: DumpStats::default(),
            version_num,
        };
        dumper
            .out
            .header(DatabaseEngine::Postgres, &version, database)
            .await?;
        dumper.run().await?;
        let stats = dumper.stats;

        let _ = exec(&mut conn, "COMMIT").await;
        let _ = conn.close().await;
        Ok(stats)
    }

    async fn restore(
        &self,
        target: &DatabaseTarget,
        database: &str,
        mode: RestoreMode,
        input: &mut LineReader,
        progress: &dyn Progress,
    ) -> Result<RestoreStats, DatabaseError> {
        let name = validate_database_name(database)?;
        let quoted = quote_ident(FAMILY, name);

        let mut admin = connect(target, maintenance(target)).await?;
        let exists = database_exists(&mut admin, name).await?;
        match (mode, exists) {
            (RestoreMode::NewDatabase, true) => {
                return Err(DatabaseError::Conflict(format!(
                    "Já existe um banco \"{name}\" — escolha outro nome ou substitua o existente"
                )))
            }
            (RestoreMode::Replace, false) => {
                return Err(DatabaseError::Conflict(format!(
                    "O banco \"{name}\" não existe para ser substituído"
                )))
            }
            (RestoreMode::NewDatabase, false) => {
                exec(&mut admin, &format!("CREATE DATABASE {quoted}")).await?;
            }
            (RestoreMode::Replace, true) => {}
        }

        let result = restore_into(target, name, mode, input, progress).await;
        if result.is_err() && mode == RestoreMode::NewDatabase {
            // Banco criado só para esta restauração: não fica lixo para trás.
            let _ = exec(&mut admin, &format!("DROP DATABASE IF EXISTS {quoted}")).await;
        }
        let _ = admin.close().await;
        result
    }
}

/// Restaura numa transação só: ou o banco fica inteiro com o backup, ou fica
/// como estava — inclusive no modo substituir, porque apagar os esquemas
/// antigos faz parte da mesma transação.
async fn restore_into(
    target: &DatabaseTarget,
    database: &str,
    mode: RestoreMode,
    input: &mut LineReader,
    progress: &dyn Progress,
) -> Result<RestoreStats, DatabaseError> {
    let mut conn = connect(target, database).await?;
    exec(&mut conn, "BEGIN").await?;

    if mode == RestoreMode::Replace {
        progress.stage("Apagando o conteúdo atual");
        let schemas: Vec<String> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT format('%I', n.nspname) FROM pg_namespace n WHERE {USER_NAMESPACES} \
             AND {} ORDER BY n.nspname",
            not_from_extension("pg_namespace", "n.oid")
        )))
        .fetch_all(&mut conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        for schema in schemas {
            exec(&mut conn, &format!("DROP SCHEMA {schema} CASCADE")).await?;
        }
    }

    let mut session = PgSession { conn };
    let stats = apply_script(&mut session, FAMILY, input, progress).await?;
    exec(&mut session.conn, "COMMIT").await?;
    let _ = session.conn.close().await;
    Ok(stats)
}

struct PgSession {
    conn: PgConnection,
}

/// Tamanho do lote enviado ao `COPY FROM`.
const COPY_BATCH_BYTES: usize = 256 * 1024;

#[async_trait]
impl RestoreSession for PgSession {
    async fn execute(&mut self, statement: &str) -> Result<u64, DatabaseError> {
        exec(&mut self.conn, statement).await
    }

    async fn copy_from(
        &mut self,
        statement: &str,
        input: &mut LineReader,
    ) -> Result<u64, DatabaseError> {
        let mut copy = self
            .conn
            .copy_in_raw(statement)
            .await
            .map_err(|error| DatabaseError::query(&error))?;
        let mut batch = Vec::with_capacity(COPY_BATCH_BYTES);
        let mut rows = 0_u64;
        loop {
            let Some(line) = input.next_line().await? else {
                let _ = copy.abort("fim do arquivo dentro de um COPY").await;
                return Err(DatabaseError::Io(
                    "o arquivo acabou no meio dos dados de uma tabela".to_string(),
                ));
            };
            if line == "\\." {
                break;
            }
            batch.extend_from_slice(line.as_bytes());
            batch.push(b'\n');
            rows += 1;
            if batch.len() >= COPY_BATCH_BYTES {
                copy.send(std::mem::take(&mut batch))
                    .await
                    .map_err(|error| DatabaseError::query(&error))?;
            }
        }
        if !batch.is_empty() {
            copy.send(batch)
                .await
                .map_err(|error| DatabaseError::query(&error))?;
        }
        copy.finish()
            .await
            .map_err(|error| DatabaseError::query(&error))?;
        Ok(rows)
    }
}

/// Um dump em andamento.
struct Dumper<'a> {
    conn: &'a mut PgConnection,
    out: &'a mut DumpWriter,
    progress: &'a dyn Progress,
    stats: DumpStats,
    version_num: i32,
}

/// Tabela a copiar.
struct TableInfo {
    oid: i64,
    qname: String,
    kind: String,
}

impl Dumper<'_> {
    async fn run(&mut self) -> Result<(), DatabaseError> {
        for statement in [
            "SET statement_timeout = 0",
            "SET client_encoding = 'UTF8'",
            "SET standard_conforming_strings = on",
            "SET check_function_bodies = false",
            "SET client_min_messages = warning",
        ] {
            self.out.statement(statement).await?;
        }

        self.schemas().await?;
        self.extensions().await?;
        self.types().await?;
        self.sequences_create().await?;
        self.functions(false).await?;
        let tables = self.tables().await?;
        self.functions(true).await?;
        self.data(&tables).await?;
        self.sequence_values().await?;
        self.constraints(false).await?;
        self.indexes("'r', 'p'").await?;
        self.constraints(true).await?;
        self.sequences_owned().await?;
        self.views().await?;
        self.triggers().await?;
        self.warnings().await?;
        self.progress.bytes(self.out.bytes());
        Ok(())
    }

    async fn schemas(&mut self) -> Result<(), DatabaseError> {
        self.progress.stage("Esquemas");
        let list = rows(
            self.conn,
            &format!(
                "SELECT format('%I', n.nspname) AS q FROM pg_namespace n \
                 WHERE {USER_NAMESPACES} AND {} ORDER BY n.nspname",
                not_from_extension("pg_namespace", "n.oid")
            ),
        )
        .await?;
        self.out.section("Esquemas").await?;
        for row in &list {
            let name: String = get(row, "q")?;
            self.out
                .statement(&format!("CREATE SCHEMA IF NOT EXISTS {name}"))
                .await?;
        }
        Ok(())
    }

    async fn extensions(&mut self) -> Result<(), DatabaseError> {
        let list = rows(
            self.conn,
            "SELECT format('%I', e.extname) AS q, format('%I', n.nspname) AS s \
             FROM pg_extension e JOIN pg_namespace n ON n.oid = e.extnamespace \
             WHERE e.extname <> 'plpgsql' ORDER BY e.oid",
        )
        .await?;
        if list.is_empty() {
            return Ok(());
        }
        self.out.section("Extensões").await?;
        for row in &list {
            let name: String = get(row, "q")?;
            let schema: String = get(row, "s")?;
            self.out
                .statement(&format!(
                    "CREATE EXTENSION IF NOT EXISTS {name} WITH SCHEMA {schema}"
                ))
                .await?;
        }
        Ok(())
    }

    async fn types(&mut self) -> Result<(), DatabaseError> {
        self.progress.stage("Tipos");
        let enums = rows(
            self.conn,
            &format!(
                "SELECT format('%I.%I', n.nspname, t.typname) AS q, \
                 array_agg(e.enumlabel::text ORDER BY e.enumsortorder) AS labels \
                 FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
                 JOIN pg_enum e ON e.enumtypid = t.oid \
                 WHERE {USER_NAMESPACES} AND {} GROUP BY n.nspname, t.typname, t.oid ORDER BY t.oid",
                not_from_extension("pg_type", "t.oid")
            ),
        )
        .await?;
        let domains = rows(
            self.conn,
            &format!(
                "SELECT format('%I.%I', n.nspname, t.typname) AS q, \
                 format_type(t.typbasetype, t.typtypmod) AS base, t.typnotnull AS notnull, \
                 t.typdefault AS def, \
                 (SELECT string_agg('CONSTRAINT ' || quote_ident(c.conname) || ' ' || \
                   pg_get_constraintdef(c.oid), ' ' ORDER BY c.conname) \
                  FROM pg_constraint c WHERE c.contypid = t.oid) AS checks \
                 FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
                 WHERE t.typtype = 'd' AND {USER_NAMESPACES} AND {} ORDER BY t.oid",
                not_from_extension("pg_type", "t.oid")
            ),
        )
        .await?;
        if enums.is_empty() && domains.is_empty() {
            return Ok(());
        }
        self.out.section("Tipos").await?;
        for row in &enums {
            let name: String = get(row, "q")?;
            let labels: Vec<String> = get(row, "labels")?;
            let labels: Vec<String> = labels
                .iter()
                .map(|label| quote_literal(FAMILY, label))
                .collect();
            self.out
                .statement(&format!(
                    "CREATE TYPE {name} AS ENUM ({})",
                    labels.join(", ")
                ))
                .await?;
        }
        for row in &domains {
            let name: String = get(row, "q")?;
            let base: String = get(row, "base")?;
            let mut sql = format!("CREATE DOMAIN {name} AS {base}");
            if let Some(default) = get::<Option<String>>(row, "def")? {
                sql.push_str(&format!(" DEFAULT {default}"));
            }
            if get::<bool>(row, "notnull")? {
                sql.push_str(" NOT NULL");
            }
            if let Some(checks) = get::<Option<String>>(row, "checks")? {
                sql.push(' ');
                sql.push_str(&checks);
            }
            self.out.statement(&sql).await?;
        }
        Ok(())
    }

    /// Sequências avulsas e de `serial` (as de identidade nascem com a coluna).
    fn sequences_query(&self) -> String {
        format!(
            "SELECT format('%I.%I', n.nspname, c.relname) AS q, \
             format_type(s.seqtypid, NULL) AS data_type, s.seqstart AS start, \
             s.seqmin AS min, s.seqmax AS max, s.seqincrement AS inc, \
             s.seqcache AS cache, s.seqcycle AS cycle, \
             (SELECT format('%I.%I', tn.nspname, tc.relname) || '.' || quote_ident(a.attname) \
              FROM pg_depend d JOIN pg_class tc ON tc.oid = d.refobjid \
              JOIN pg_namespace tn ON tn.oid = tc.relnamespace \
              JOIN pg_attribute a ON a.attrelid = tc.oid AND a.attnum = d.refobjsubid \
              WHERE d.classid = 'pg_class'::regclass AND d.objid = c.oid \
              AND d.refclassid = 'pg_class'::regclass AND d.deptype = 'a' \
              AND d.refobjsubid > 0 LIMIT 1) AS owned_by \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_sequence s ON s.seqrelid = c.oid \
             WHERE c.relkind = 'S' AND {USER_NAMESPACES} AND {} \
             AND NOT EXISTS (SELECT 1 FROM pg_depend d WHERE d.classid = 'pg_class'::regclass \
               AND d.objid = c.oid AND d.deptype = 'i') ORDER BY c.oid",
            not_from_extension("pg_class", "c.oid")
        )
    }

    async fn sequences_create(&mut self) -> Result<(), DatabaseError> {
        let list = rows(self.conn, &self.sequences_query()).await?;
        if list.is_empty() {
            return Ok(());
        }
        self.out.section("Sequências").await?;
        for row in &list {
            let name: String = get(row, "q")?;
            let data_type: String = get(row, "data_type")?;
            let cycle = if get::<bool>(row, "cycle")? {
                "CYCLE"
            } else {
                "NO CYCLE"
            };
            self.out
                .statement(&format!(
                    "CREATE SEQUENCE IF NOT EXISTS {name} AS {data_type} INCREMENT BY {} \
                     MINVALUE {} MAXVALUE {} START WITH {} CACHE {} {cycle}",
                    get::<i64>(row, "inc")?,
                    get::<i64>(row, "min")?,
                    get::<i64>(row, "max")?,
                    get::<i64>(row, "start")?,
                    get::<i64>(row, "cache")?,
                ))
                .await?;
        }
        Ok(())
    }

    /// Valor atual de cada sequência, depois dos dados.
    async fn sequence_values(&mut self) -> Result<(), DatabaseError> {
        let standalone = rows(self.conn, &self.sequences_query()).await?;
        let identity = rows(
            self.conn,
            &format!(
                "SELECT format('%I.%I', sn.nspname, s.relname) AS seq, \
                 format('%I.%I', n.nspname, c.relname) AS tbl, a.attname::text AS col \
                 FROM pg_class s JOIN pg_namespace sn ON sn.oid = s.relnamespace \
                 JOIN pg_depend d ON d.classid = 'pg_class'::regclass AND d.objid = s.oid \
                   AND d.deptype = 'i' AND d.refclassid = 'pg_class'::regclass \
                 JOIN pg_class c ON c.oid = d.refobjid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = d.refobjsubid \
                 WHERE s.relkind = 'S' AND {USER_NAMESPACES} ORDER BY s.oid"
            ),
        )
        .await?;
        if standalone.is_empty() && identity.is_empty() {
            return Ok(());
        }
        self.out.section("Valores das sequências").await?;
        for row in &standalone {
            let name: String = get(row, "q")?;
            let (value, called) = self.sequence_state(&name).await?;
            self.out
                .statement(&format!(
                    "SELECT pg_catalog.setval({}, {value}, {called})",
                    quote_literal(FAMILY, &name)
                ))
                .await?;
        }
        for row in &identity {
            let sequence: String = get(row, "seq")?;
            let table: String = get(row, "tbl")?;
            let column: String = get(row, "col")?;
            let (value, called) = self.sequence_state(&sequence).await?;
            self.out
                .statement(&format!(
                    "SELECT pg_catalog.setval(pg_catalog.pg_get_serial_sequence({}, {}), {value}, {called})",
                    quote_literal(FAMILY, &table),
                    quote_literal(FAMILY, &column)
                ))
                .await?;
        }
        Ok(())
    }

    async fn sequence_state(&mut self, qname: &str) -> Result<(i64, bool), DatabaseError> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT last_value, is_called FROM {qname}"
        )))
        .fetch_one(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        Ok((get(&row, "last_value")?, get(&row, "is_called")?))
    }

    async fn sequences_owned(&mut self) -> Result<(), DatabaseError> {
        let list = rows(self.conn, &self.sequences_query()).await?;
        for row in &list {
            if let Some(owner) = get::<Option<String>>(row, "owned_by")? {
                let name: String = get(row, "q")?;
                self.out
                    .statement(&format!("ALTER SEQUENCE {name} OWNED BY {owner}"))
                    .await?;
            }
        }
        Ok(())
    }

    /// Funções e procedures. As que usam o tipo de uma tabela na assinatura
    /// (`RETURNS SETOF clientes`) só podem nascer depois das tabelas.
    async fn functions(&mut self, uses_table_types: bool) -> Result<(), DatabaseError> {
        let list = rows(
            self.conn,
            &format!(
                "SELECT pg_get_functiondef(p.oid) AS def FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE {USER_NAMESPACES} AND p.prokind IN ('f', 'p') AND {} \
                 AND {} EXISTS (SELECT 1 FROM pg_depend d JOIN pg_type t \
                   ON d.refclassid = 'pg_type'::regclass AND d.refobjid = t.oid \
                   WHERE d.classid = 'pg_proc'::regclass AND d.objid = p.oid AND t.typrelid <> 0) \
                 ORDER BY p.oid",
                not_from_extension("pg_proc", "p.oid"),
                if uses_table_types { "" } else { "NOT" }
            ),
        )
        .await?;
        if list.is_empty() {
            return Ok(());
        }
        self.progress.stage("Funções");
        self.out
            .section(if uses_table_types {
                "Funções que usam tipos de tabela"
            } else {
                "Funções"
            })
            .await?;
        for row in &list {
            let def: String = get(row, "def")?;
            self.out.statement(&def).await?;
        }
        Ok(())
    }

    async fn tables(&mut self) -> Result<Vec<TableInfo>, DatabaseError> {
        let list = rows(
            self.conn,
            &format!(
                "SELECT c.oid::bigint AS oid, format('%I.%I', n.nspname, c.relname) AS q, \
                 c.relkind::text AS kind, c.relpersistence::text AS persistence, \
                 c.relispartition AS is_partition, \
                 CASE WHEN c.relkind = 'p' THEN pg_get_partkeydef(c.oid) END AS partkey, \
                 CASE WHEN c.relispartition THEN (SELECT format('%I.%I', pn.nspname, pc.relname) \
                   FROM pg_inherits i JOIN pg_class pc ON pc.oid = i.inhparent \
                   JOIN pg_namespace pn ON pn.oid = pc.relnamespace WHERE i.inhrelid = c.oid) END AS parent, \
                 CASE WHEN c.relispartition THEN pg_get_expr(c.relpartbound, c.oid) END AS bound \
                 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE c.relkind IN ('r', 'p') AND {USER_NAMESPACES} AND {} \
                 ORDER BY c.relispartition, c.oid",
                not_from_extension("pg_class", "c.oid")
            ),
        )
        .await?;
        self.out.section("Tabelas").await?;
        let mut tables = Vec::with_capacity(list.len());
        for row in &list {
            let table = TableInfo {
                oid: get(row, "oid")?,
                qname: get(row, "q")?,
                kind: get(row, "kind")?,
            };
            self.progress
                .stage(&format!("Estrutura de {}", table.qname));
            let unlogged = get::<String>(row, "persistence")? == "u";
            let create = if unlogged {
                "CREATE UNLOGGED TABLE"
            } else {
                "CREATE TABLE"
            };
            let sql = if get::<bool>(row, "is_partition")? {
                let parent: String = get(row, "parent")?;
                let bound: String = get(row, "bound")?;
                format!("{create} {} PARTITION OF {parent} {bound}", table.qname)
            } else {
                let columns = self.column_definitions(table.oid).await?;
                let mut sql = format!(
                    "{create} {} (\n    {}\n)",
                    table.qname,
                    columns.join(",\n    ")
                );
                if let Some(partkey) = get::<Option<String>>(row, "partkey")? {
                    sql.push_str(&format!(" PARTITION BY {partkey}"));
                }
                sql
            };
            self.out.statement(&sql).await?;
            self.stats.tables += 1;
            tables.push(table);
        }
        Ok(tables)
    }

    async fn column_definitions(&mut self, oid: i64) -> Result<Vec<String>, DatabaseError> {
        let list = sqlx::query(
            "SELECT quote_ident(a.attname) AS q, format_type(a.atttypid, a.atttypmod) AS typ, \
             a.attnotnull AS notnull, pg_get_expr(ad.adbin, ad.adrelid) AS def, \
             a.attidentity::text AS identity, a.attgenerated::text AS generated, \
             CASE WHEN a.attcollation <> 0 AND a.attcollation <> t.typcollation THEN \
               (SELECT format('%I.%I', cn.nspname, co.collname) FROM pg_collation co \
                JOIN pg_namespace cn ON cn.oid = co.collnamespace WHERE co.oid = a.attcollation) \
             END AS collation \
             FROM pg_attribute a JOIN pg_type t ON t.oid = a.atttypid \
             LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum \
             WHERE a.attrelid = $1::bigint::oid AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY a.attnum",
        )
        .bind(oid)
        .fetch_all(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;

        list.iter()
            .map(|row| {
                let mut column = format!(
                    "{} {}",
                    get::<String>(row, "q")?,
                    get::<String>(row, "typ")?
                );
                if let Some(collation) = get::<Option<String>>(row, "collation")? {
                    column.push_str(&format!(" COLLATE {collation}"));
                }
                let def: Option<String> = get(row, "def")?;
                match (
                    get::<String>(row, "generated")?.as_str(),
                    get::<String>(row, "identity")?.as_str(),
                ) {
                    ("s", _) => column.push_str(&format!(
                        " GENERATED ALWAYS AS ({}) STORED",
                        def.unwrap_or_default()
                    )),
                    (_, "a") => column.push_str(" GENERATED ALWAYS AS IDENTITY"),
                    (_, "d") => column.push_str(" GENERATED BY DEFAULT AS IDENTITY"),
                    _ => {
                        if let Some(def) = def {
                            column.push_str(&format!(" DEFAULT {def}"));
                        }
                    }
                }
                if get::<bool>(row, "notnull")? {
                    column.push_str(" NOT NULL");
                }
                Ok(column)
            })
            .collect()
    }

    async fn data(&mut self, tables: &[TableInfo]) -> Result<(), DatabaseError> {
        self.out.section("Dados").await?;
        for table in tables.iter().filter(|table| table.kind == "r") {
            let columns: Vec<String> = sqlx::query_scalar(
                "SELECT quote_ident(attname) FROM pg_attribute \
                 WHERE attrelid = $1::bigint::oid AND attnum > 0 AND NOT attisdropped \
                 AND attgenerated = '' ORDER BY attnum",
            )
            .bind(table.oid)
            .fetch_all(&mut *self.conn)
            .await
            .map_err(|error| DatabaseError::query(&error))?;
            if columns.is_empty() {
                continue;
            }
            self.progress.stage(&format!("Dados de {}", table.qname));
            let list = columns.join(", ");
            self.out
                .line(&format!("COPY {} ({list}) FROM stdin;", table.qname))
                .await?;

            let mut stream = self
                .conn
                .copy_out_raw(&format!("COPY {} ({list}) TO STDOUT", table.qname))
                .await
                .map_err(|error| DatabaseError::query(&error))?;
            let mut rows = 0_u64;
            while let Some(chunk) = stream
                .try_next()
                .await
                .map_err(|error| DatabaseError::query(&error))?
            {
                let lines = chunk.iter().filter(|byte| **byte == b'\n').count() as u64;
                rows += lines;
                self.out.write(&chunk).await?;
                self.progress.rows(lines);
                self.progress.bytes(self.out.bytes());
            }
            drop(stream);
            self.out.line("\\.").await?;
            self.stats.rows += rows;
        }
        Ok(())
    }

    /// Chaves primárias, únicas, exclusão e checks — ou só as FKs.
    async fn constraints(&mut self, foreign: bool) -> Result<(), DatabaseError> {
        let kinds = if foreign { "'f'" } else { "'p', 'u', 'x', 'c'" };
        let list = rows(
            self.conn,
            &format!(
                "SELECT format('%I.%I', n.nspname, cl.relname) AS tbl, cl.relkind::text AS kind, \
                 quote_ident(co.conname) AS q, pg_get_constraintdef(co.oid) AS def \
                 FROM pg_constraint co JOIN pg_class cl ON cl.oid = co.conrelid \
                 JOIN pg_namespace n ON n.oid = cl.relnamespace \
                 WHERE co.contype IN ({kinds}) AND cl.relkind IN ('r', 'p') \
                 AND co.conparentid = 0 AND co.conislocal \
                 AND {USER_NAMESPACES} AND {} \
                 ORDER BY CASE co.contype WHEN 'p' THEN 0 WHEN 'u' THEN 1 WHEN 'x' THEN 2 ELSE 3 END, co.oid",
                not_from_extension("pg_class", "cl.oid")
            ),
        )
        .await?;
        if list.is_empty() {
            return Ok(());
        }
        self.progress.stage(if foreign {
            "Chaves estrangeiras"
        } else {
            "Chaves e checks"
        });
        self.out
            .section(if foreign {
                "Chaves estrangeiras"
            } else {
                "Chaves e checks"
            })
            .await?;
        for row in &list {
            // Em tabela particionada a restrição precisa descer às partições;
            // `ONLY` a deixaria só no pai.
            let only = if get::<String>(row, "kind")? == "p" {
                ""
            } else {
                "ONLY "
            };
            self.out
                .statement(&format!(
                    "ALTER TABLE {only}{} ADD CONSTRAINT {} {}",
                    get::<String>(row, "tbl")?,
                    get::<String>(row, "q")?,
                    get::<String>(row, "def")?
                ))
                .await?;
        }
        Ok(())
    }

    /// Índices que não sustentam uma restrição (esses nascem com ela).
    async fn indexes(&mut self, relkinds: &str) -> Result<(), DatabaseError> {
        let list = rows(
            self.conn,
            &format!(
                "SELECT pg_get_indexdef(i.indexrelid) AS def FROM pg_index i \
                 JOIN pg_class ic ON ic.oid = i.indexrelid \
                 JOIN pg_class tc ON tc.oid = i.indrelid \
                 JOIN pg_namespace n ON n.oid = tc.relnamespace \
                 WHERE tc.relkind IN ({relkinds}) AND {USER_NAMESPACES} AND {} \
                 AND NOT EXISTS (SELECT 1 FROM pg_constraint co WHERE co.conindid = i.indexrelid \
                   AND co.contype IN ('p', 'u', 'x')) \
                 AND NOT EXISTS (SELECT 1 FROM pg_inherits inh WHERE inh.inhrelid = i.indexrelid) \
                 ORDER BY ic.oid",
                not_from_extension("pg_class", "tc.oid")
            ),
        )
        .await?;
        if list.is_empty() {
            return Ok(());
        }
        self.progress.stage("Índices");
        self.out.section("Índices").await?;
        for row in &list {
            let def: String = get(row, "def")?;
            self.out.statement(&def).await?;
        }
        Ok(())
    }

    async fn views(&mut self) -> Result<(), DatabaseError> {
        let list = rows(
            self.conn,
            &format!(
                "SELECT c.oid::bigint AS oid, format('%I.%I', n.nspname, c.relname) AS q, \
                 c.relkind::text AS kind, c.relispopulated AS populated, \
                 pg_get_viewdef(c.oid, true) AS def, \
                 ARRAY(SELECT DISTINCT d.refobjid::bigint FROM pg_rewrite r \
                   JOIN pg_depend d ON d.classid = 'pg_rewrite'::regclass AND d.objid = r.oid \
                   AND d.refclassid = 'pg_class'::regclass \
                   WHERE r.ev_class = c.oid AND d.refobjid <> c.oid) AS deps \
                 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE c.relkind IN ('v', 'm') AND {USER_NAMESPACES} AND {} ORDER BY c.oid",
                not_from_extension("pg_class", "c.oid")
            ),
        )
        .await?;
        if list.is_empty() {
            return Ok(());
        }
        struct View {
            oid: i64,
            qname: String,
            materialized: bool,
            populated: bool,
            def: String,
            deps: Vec<i64>,
        }
        let views = list
            .iter()
            .map(|row| {
                Ok(View {
                    oid: get(row, "oid")?,
                    qname: get(row, "q")?,
                    materialized: get::<String>(row, "kind")? == "m",
                    populated: get(row, "populated")?,
                    def: get(row, "def")?,
                    deps: get(row, "deps")?,
                })
            })
            .collect::<Result<Vec<_>, DatabaseError>>()?;
        let oids: Vec<i64> = views.iter().map(|view| view.oid).collect();
        let deps: Vec<Vec<usize>> = views
            .iter()
            .map(|view| {
                view.deps
                    .iter()
                    .filter_map(|dep| oids.iter().position(|oid| oid == dep))
                    .collect()
            })
            .collect();
        let views = dependency_order(views, |index| deps[index].clone());

        self.progress.stage("Views");
        self.out.section("Views").await?;
        for view in &views {
            let body = view.def.trim().trim_end_matches(';');
            let sql = if view.materialized {
                format!(
                    "CREATE MATERIALIZED VIEW {} AS\n{body}\nWITH NO DATA",
                    view.qname
                )
            } else {
                format!("CREATE VIEW {} AS\n{body}", view.qname)
            };
            self.out.statement(&sql).await?;
        }
        self.indexes("'m'").await?;
        for view in views
            .iter()
            .filter(|view| view.materialized && view.populated)
        {
            self.out
                .statement(&format!("REFRESH MATERIALIZED VIEW {}", view.qname))
                .await?;
        }
        Ok(())
    }

    async fn triggers(&mut self) -> Result<(), DatabaseError> {
        // Partições herdam o trigger do pai (PostgreSQL 13+): o clone não vai.
        let clone_filter = if self.version_num >= 130_000 {
            "AND t.tgparentid = 0"
        } else {
            ""
        };
        let list = rows(
            self.conn,
            &format!(
                "SELECT pg_get_triggerdef(t.oid) AS def FROM pg_trigger t \
                 JOIN pg_class c ON c.oid = t.tgrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE NOT t.tgisinternal {clone_filter} AND {USER_NAMESPACES} AND {} ORDER BY t.oid",
                not_from_extension("pg_class", "c.oid")
            ),
        )
        .await?;
        if list.is_empty() {
            return Ok(());
        }
        self.out.section("Triggers").await?;
        for row in &list {
            let def: String = get(row, "def")?;
            self.out.statement(&def).await?;
        }
        Ok(())
    }

    /// Conta o que existe e o dump não leva.
    async fn warnings(&mut self) -> Result<(), DatabaseError> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT \
             (SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
              WHERE p.prokind IN ('a', 'w') AND {USER_NAMESPACES} AND {proc}) AS aggregates, \
             (SELECT count(*) FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
              WHERE (t.typtype IN ('r', 'm') OR (t.typtype = 'c' AND EXISTS \
                (SELECT 1 FROM pg_class c WHERE c.oid = t.typrelid AND c.relkind = 'c'))) \
              AND {USER_NAMESPACES} AND {typ}) AS types, \
             (SELECT count(*) FROM pg_rewrite r JOIN pg_class c ON c.oid = r.ev_class \
              JOIN pg_namespace n ON n.oid = c.relnamespace \
              WHERE r.rulename <> '_RETURN' AND {USER_NAMESPACES}) AS rules, \
             (SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
              WHERE c.relkind = 'f' AND {USER_NAMESPACES}) AS foreign_tables, \
             (SELECT count(*) FROM pg_largeobject_metadata) AS large_objects",
            proc = not_from_extension("pg_proc", "p.oid"),
            typ = not_from_extension("pg_type", "t.oid"),
        )))
        .fetch_one(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;

        for (column, label) in [
            ("aggregates", "agregado(s) e funções de janela"),
            ("types", "tipo(s) composto(s) ou de intervalo"),
            ("rules", "regra(s) (CREATE RULE)"),
            ("foreign_tables", "tabela(s) estrangeira(s)"),
            ("large_objects", "objeto(s) grande(s) (large objects)"),
        ] {
            let count: i64 = get(&row, column)?;
            if count > 0 {
                self.stats
                    .warnings
                    .push(format!("{count} {label} não entram no backup"));
            }
        }
        Ok(())
    }
}
