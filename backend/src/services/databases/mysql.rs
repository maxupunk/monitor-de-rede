//! MySQL e MariaDB: dump por `SHOW CREATE` e `INSERT` estendido.
//!
//! ## A estrutura é a do próprio servidor
//!
//! `SHOW CREATE TABLE/VIEW/PROCEDURE/FUNCTION/TRIGGER` devolve o DDL exato, e é
//! ele que vai para o arquivo. A conexão do dump usa o banco como padrão: assim
//! o servidor omite o nome do banco nas views e rotinas, e o dump restaura em
//! um banco de outro nome. O `DEFINER` sai — ele amarraria o objeto a um
//! usuário que pode não existir no servidor de destino.
//!
//! ## Os valores são citados pelo servidor
//!
//! Cada linha volta como **um** texto já pronto para o `VALUES`, montado no
//! `SELECT`: `QUOTE(coluna)` para texto, número, data e JSON, e `0x` + `HEX()`
//! para binário e geometria. Nenhum tipo é convertido aqui — e por isso não há
//! tipo que este módulo "não conheça". `QUOTE(NULL)` já devolve `NULL`.
//!
//! ## Foto consistente
//!
//! `START TRANSACTION WITH CONSISTENT SNAPSHOT` em `REPEATABLE READ`: tabelas
//! InnoDB saem de um instante só, sem travar a escrita (é o `--single-transaction`
//! do `mysqldump`). MyISAM não tem transação — é o mesmo limite do `mysqldump`.

use async_trait::async_trait;
use futures::TryStreamExt;
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlRow, MySqlSslMode},
    AssertSqlSafe, Connection, MySqlConnection, Row,
};

use super::{
    connect_with_timeout,
    dump::{DumpStats, DumpWriter},
    restore::{apply_script, LineReader, RestoreSession, RestoreStats},
    sql::{order::dependency_order, quote_ident, validate_database_name},
    DatabaseEngine, DatabaseError, DatabaseTarget, Driver, EngineFamily, Probe, Progress,
    RestoreMode, SslMode,
};

const FAMILY: EngineFamily = EngineFamily::Mysql;

/// Bancos do próprio servidor, fora da lista.
const SYSTEM_DATABASES: [&str; 4] = ["information_schema", "mysql", "performance_schema", "sys"];

/// Teto de um `INSERT` estendido. Bem abaixo do `max_allowed_packet` padrão
/// (4 MB nos servidores antigos): o restore não pode esbarrar nele.
const INSERT_BATCH_BYTES: usize = 512 * 1024;

pub struct MysqlDriver;

async fn connect(
    target: &DatabaseTarget,
    database: Option<&str>,
) -> Result<MySqlConnection, DatabaseError> {
    let mut options = MySqlConnectOptions::new()
        .host(&target.host)
        .port(target.port)
        .username(&target.username)
        .password(&target.password)
        .charset("utf8mb4")
        .ssl_mode(match target.ssl_mode {
            SslMode::Disable => MySqlSslMode::Disabled,
            SslMode::Prefer => MySqlSslMode::Preferred,
            SslMode::Require => MySqlSslMode::Required,
        });
    if let Some(database) = database {
        options = options.database(database);
    }
    connect_with_timeout(target, MySqlConnection::connect_with(&options)).await
}

async fn exec(conn: &mut MySqlConnection, sql: &str) -> Result<u64, DatabaseError> {
    sqlx::raw_sql(AssertSqlSafe(sql.to_string()))
        .execute(conn)
        .await
        .map(|done| done.rows_affected())
        .map_err(|error| DatabaseError::query(&error))
}

/// Texto de uma coluna. O MySQL devolve parte do `information_schema` e do
/// `SHOW CREATE` como binário conforme a versão; os dois chegam aqui.
fn text(row: &MySqlRow, index: usize) -> Result<Option<String>, DatabaseError> {
    if let Ok(value) = row.try_get::<Option<String>, _>(index) {
        return Ok(value);
    }
    row.try_get::<Option<Vec<u8>>, _>(index)
        .map(|value| value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
        .map_err(|error| DatabaseError::Query(format!("coluna {index}: {error}")))
}

fn required_text(row: &MySqlRow, index: usize) -> Result<String, DatabaseError> {
    text(row, index)?.ok_or_else(|| {
        DatabaseError::Query("o servidor não devolveu a definição — falta privilégio?".to_string())
    })
}

#[async_trait]
impl Driver for MysqlDriver {
    async fn probe(&self, target: &DatabaseTarget) -> Result<Probe, DatabaseError> {
        let started = std::time::Instant::now();
        let mut conn = connect(target, target.database.as_deref()).await?;
        let row = sqlx::query("SELECT VERSION()")
            .fetch_one(&mut conn)
            .await
            .map_err(|error| DatabaseError::query(&error))?;
        let version = required_text(&row, 0)?;
        let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
        let list = sqlx::query("SHOW DATABASES")
            .fetch_all(&mut conn)
            .await
            .map_err(|error| DatabaseError::query(&error))?;
        let databases = list
            .iter()
            .filter_map(|row| text(row, 0).ok().flatten())
            .filter(|name| !SYSTEM_DATABASES.contains(&name.as_str()))
            .collect();
        let _ = conn.close().await;
        let label = if version.to_ascii_lowercase().contains("mariadb") {
            "MariaDB"
        } else {
            "MySQL"
        };
        Ok(Probe {
            latency_ms,
            version: format!("{label} {version}"),
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
        let mut conn = connect(target, Some(database)).await?;
        let row = sqlx::query("SELECT VERSION()")
            .fetch_one(&mut conn)
            .await
            .map_err(|error| DatabaseError::query(&error))?;
        let version = required_text(&row, 0)?;
        // `sql_mode` previsível: com ANSI_QUOTES o `SHOW CREATE` citaria com
        // aspas duplas, e o arquivo não restauraria num servidor sem ele.
        exec(
            &mut conn,
            "SET SESSION sql_mode = 'NO_ENGINE_SUBSTITUTION', time_zone = '+00:00'",
        )
        .await?;
        exec(
            &mut conn,
            "SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ",
        )
        .await?;
        exec(&mut conn, "START TRANSACTION WITH CONSISTENT SNAPSHOT").await?;

        let engine = if version.to_ascii_lowercase().contains("mariadb") {
            DatabaseEngine::Mariadb
        } else {
            DatabaseEngine::Mysql
        };
        out.header(engine, &version, database).await?;
        let mut dumper = Dumper {
            conn: &mut conn,
            out,
            progress,
            stats: DumpStats::default(),
        };
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
        let mut admin = connect(target, None).await?;
        let exists = sqlx::query("SELECT 1 FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?")
            .bind(name)
            .fetch_optional(&mut admin)
            .await
            .map_err(|error| DatabaseError::query(&error))?
            .is_some();
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
            (RestoreMode::Replace, true) => {
                // DDL no MySQL não volta atrás: substituir é apagar de fato. A
                // tela diz isso e exige que o operador digite o nome.
                progress.stage("Apagando o conteúdo atual");
                exec(&mut admin, &format!("DROP DATABASE {quoted}")).await?;
                exec(
                    &mut admin,
                    &format!("CREATE DATABASE {quoted} CHARACTER SET utf8mb4"),
                )
                .await?;
            }
            (RestoreMode::NewDatabase, false) => {
                exec(
                    &mut admin,
                    &format!("CREATE DATABASE {quoted} CHARACTER SET utf8mb4"),
                )
                .await?;
            }
        }

        let result = async {
            let conn = connect(target, Some(name)).await?;
            let mut session = MysqlSession { conn };
            let stats = apply_script(&mut session, FAMILY, input, progress).await?;
            let _ = session.conn.close().await;
            Ok(stats)
        }
        .await;
        if result.is_err() && mode == RestoreMode::NewDatabase {
            let _ = exec(&mut admin, &format!("DROP DATABASE IF EXISTS {quoted}")).await;
        }
        let _ = admin.close().await;
        result
    }
}

struct MysqlSession {
    conn: MySqlConnection,
}

#[async_trait]
impl RestoreSession for MysqlSession {
    async fn execute(&mut self, statement: &str) -> Result<u64, DatabaseError> {
        exec(&mut self.conn, statement).await
    }
}

struct Dumper<'a> {
    conn: &'a mut MySqlConnection,
    out: &'a mut DumpWriter,
    progress: &'a dyn Progress,
    stats: DumpStats,
}

impl Dumper<'_> {
    async fn run(&mut self) -> Result<(), DatabaseError> {
        for statement in [
            "SET NAMES utf8mb4",
            "SET TIME_ZONE = '+00:00'",
            "SET FOREIGN_KEY_CHECKS = 0",
            "SET UNIQUE_CHECKS = 0",
            "SET SQL_MODE = 'NO_AUTO_VALUE_ON_ZERO'",
            "SET SQL_NOTES = 0",
        ] {
            self.out.statement(statement).await?;
        }

        let objects = sqlx::query(
            "SELECT TABLE_NAME, TABLE_TYPE FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = DATABASE() ORDER BY TABLE_NAME",
        )
        .fetch_all(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        let mut tables = Vec::new();
        let mut views = Vec::new();
        let mut sequences = 0;
        for row in &objects {
            let name = required_text(row, 0)?;
            match required_text(row, 1)?.as_str() {
                "BASE TABLE" | "SYSTEM VERSIONED" => tables.push(name),
                "VIEW" => views.push(name),
                _ => sequences += 1,
            }
        }
        if sequences > 0 {
            self.stats.warnings.push(format!(
                "{sequences} sequência(s) do MariaDB não entram no backup"
            ));
        }

        self.out.section("Tabelas").await?;
        for table in &tables {
            self.progress.stage(&format!("Estrutura de {table}"));
            let row = self.show_create("TABLE", table).await?;
            self.out.statement(&required_text(&row, 1)?).await?;
            self.stats.tables += 1;
        }

        self.out.section("Dados").await?;
        for table in &tables {
            self.table_data(table).await?;
        }

        self.routines().await?;
        self.views(&views).await?;
        self.triggers().await?;
        self.events_warning().await?;

        self.out.section("Fim").await?;
        self.out.statement("SET FOREIGN_KEY_CHECKS = 1").await?;
        self.out.statement("SET UNIQUE_CHECKS = 1").await?;
        self.progress.bytes(self.out.bytes());
        Ok(())
    }

    async fn show_create(&mut self, kind: &str, name: &str) -> Result<MySqlRow, DatabaseError> {
        sqlx::query(AssertSqlSafe(format!(
            "SHOW CREATE {kind} {}",
            quote_ident(FAMILY, name)
        )))
        .fetch_one(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))
    }

    async fn table_data(&mut self, table: &str) -> Result<(), DatabaseError> {
        let columns = sqlx::query(
            "SELECT COLUMN_NAME, DATA_TYPE, EXTRA FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
        )
        .bind(table)
        .fetch_all(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        let mut names = Vec::new();
        let mut values = Vec::new();
        for row in &columns {
            let extra = text(row, 2)?.unwrap_or_default();
            if is_generated(&extra) {
                continue;
            }
            let name = required_text(row, 0)?;
            let data_type = required_text(row, 1)?;
            values.push(value_expression(&quote_ident(FAMILY, &name), &data_type));
            names.push(quote_ident(FAMILY, &name));
        }
        if names.is_empty() {
            return Ok(());
        }

        self.progress.stage(&format!("Dados de {table}"));
        let qtable = quote_ident(FAMILY, table);
        let insert = format!("INSERT INTO {qtable} ({}) VALUES", names.join(", "));
        let select = format!("SELECT CONCAT_WS(',', {}) FROM {qtable}", values.join(", "));

        let mut batch = String::new();
        let mut in_batch = 0_u64;
        let mut stream = sqlx::query(AssertSqlSafe(select)).fetch(&mut *self.conn);
        while let Some(row) = stream
            .try_next()
            .await
            .map_err(|error| DatabaseError::query(&error))?
        {
            let tuple = required_text(&row, 0)?;
            if batch.is_empty() {
                batch.push_str(&insert);
                batch.push('\n');
            } else {
                batch.push_str(",\n");
            }
            batch.push('(');
            batch.push_str(&tuple);
            batch.push(')');
            in_batch += 1;
            if batch.len() >= INSERT_BATCH_BYTES {
                self.out.statement(&batch).await?;
                self.stats.rows += in_batch;
                self.progress.rows(in_batch);
                self.progress.bytes(self.out.bytes());
                batch.clear();
                in_batch = 0;
            }
        }
        drop(stream);
        if !batch.is_empty() {
            self.out.statement(&batch).await?;
            self.stats.rows += in_batch;
            self.progress.rows(in_batch);
        }
        Ok(())
    }

    async fn routines(&mut self) -> Result<(), DatabaseError> {
        let list = sqlx::query(
            "SELECT ROUTINE_NAME, ROUTINE_TYPE FROM information_schema.ROUTINES \
             WHERE ROUTINE_SCHEMA = DATABASE() ORDER BY ROUTINE_TYPE, ROUTINE_NAME",
        )
        .fetch_all(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        if list.is_empty() {
            return Ok(());
        }
        self.progress.stage("Procedures e funções");
        self.out.section("Procedures e funções").await?;
        for row in &list {
            let name = required_text(row, 0)?;
            let kind = required_text(row, 1)?;
            let create = self.show_create(&kind, &name).await?;
            match text(&create, 2)? {
                Some(sql) => self.delimited(&strip_definer(&sql)).await?,
                None => self.stats.warnings.push(format!(
                    "{} {name}: sem privilégio para ler a definição",
                    kind.to_lowercase()
                )),
            }
        }
        Ok(())
    }

    async fn views(&mut self, names: &[String]) -> Result<(), DatabaseError> {
        if names.is_empty() {
            return Ok(());
        }
        self.progress.stage("Views");
        let mut views = Vec::with_capacity(names.len());
        for name in names {
            let row = self.show_create("VIEW", name).await?;
            views.push((name.clone(), strip_definer(&required_text(&row, 1)?)));
        }
        // Sem catálogo de dependências portável entre MySQL e MariaDB: uma view
        // depende da outra se a cita pelo nome.
        let cited: Vec<String> = views
            .iter()
            .map(|(name, _)| quote_ident(FAMILY, name))
            .collect();
        let deps: Vec<Vec<usize>> = views
            .iter()
            .map(|(_, sql)| {
                let body = sql
                    .split_once(" AS ")
                    .map_or(sql.as_str(), |(_, body)| body);
                cited
                    .iter()
                    .enumerate()
                    .filter(|(_, quoted)| body.contains(quoted.as_str()))
                    .map(|(index, _)| index)
                    .collect()
            })
            .collect();
        let views = dependency_order(views, |index| deps[index].clone());

        self.out.section("Views").await?;
        for (_, sql) in &views {
            self.out.statement(sql).await?;
        }
        Ok(())
    }

    async fn triggers(&mut self) -> Result<(), DatabaseError> {
        let list = sqlx::query(
            "SELECT TRIGGER_NAME FROM information_schema.TRIGGERS \
             WHERE TRIGGER_SCHEMA = DATABASE() ORDER BY EVENT_OBJECT_TABLE, ACTION_ORDER",
        )
        .fetch_all(&mut *self.conn)
        .await
        .map_err(|error| DatabaseError::query(&error))?;
        if list.is_empty() {
            return Ok(());
        }
        self.out.section("Triggers").await?;
        for row in &list {
            let name = required_text(row, 0)?;
            let create = self.show_create("TRIGGER", &name).await?;
            self.delimited(&strip_definer(&required_text(&create, 2)?))
                .await?;
        }
        Ok(())
    }

    async fn events_warning(&mut self) -> Result<(), DatabaseError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.EVENTS WHERE EVENT_SCHEMA = DATABASE()",
        )
        .fetch_one(&mut *self.conn)
        .await
        .unwrap_or(0);
        if count > 0 {
            self.stats.warnings.push(format!(
                "{count} evento(s) agendado(s) não entram no backup"
            ));
        }
        Ok(())
    }

    /// Corpo com `;` dentro: entre `DELIMITER ;;` e `DELIMITER ;`, como o
    /// cliente `mysql` espera.
    async fn delimited(&mut self, sql: &str) -> Result<(), DatabaseError> {
        self.out.line("DELIMITER ;;").await?;
        self.out.line(&format!("{} ;;", sql.trim_end())).await?;
        self.out.line("DELIMITER ;").await
    }
}

/// Coluna gerada: o servidor a calcula, e um `INSERT` com valor nela falha.
///
/// O MySQL 8 também marca `DEFAULT_GENERATED` em coluna com default por
/// expressão (`CURRENT_TIMESTAMP`) — essa tem dado próprio e precisa ir.
#[must_use]
pub fn is_generated(extra: &str) -> bool {
    let extra = extra.to_ascii_uppercase();
    [
        "VIRTUAL GENERATED",
        "STORED GENERATED",
        "PERSISTENT GENERATED",
    ]
    .iter()
    .any(|marker| extra.contains(marker))
}

/// Expressão SQL que transforma a coluna num literal pronto para o `VALUES`.
#[must_use]
pub fn value_expression(column: &str, data_type: &str) -> String {
    const BINARY: [&str; 18] = [
        "binary",
        "varbinary",
        "tinyblob",
        "blob",
        "mediumblob",
        "longblob",
        "bit",
        "geometry",
        "point",
        "linestring",
        "polygon",
        "multipoint",
        "multilinestring",
        "multipolygon",
        "geometrycollection",
        "geomcollection",
        "vector",
        "raw",
    ];
    if BINARY.contains(&data_type.to_ascii_lowercase().as_str()) {
        // `0x` sozinho não é literal válido: o valor vazio vira ''.
        format!(
            "CASE WHEN {column} IS NULL THEN 'NULL' WHEN LENGTH({column}) = 0 THEN '''''' \
             ELSE CONCAT('0x', HEX({column})) END"
        )
    } else {
        format!("QUOTE({column})")
    }
}

/// Tira o `DEFINER=...` de um `CREATE`.
#[must_use]
pub fn strip_definer(sql: &str) -> String {
    let upper = sql.to_ascii_uppercase();
    let Some(start) = upper.find("DEFINER=") else {
        return sql.to_string();
    };
    let mut end = start + "DEFINER=".len();
    let bytes = sql.as_bytes();
    let mut quote: Option<u8> = None;
    while end < bytes.len() {
        let byte = bytes[end];
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None if byte == b'`' || byte == b'\'' || byte == b'"' => quote = Some(byte),
            None if byte.is_ascii_whitespace() => break,
            None => {}
        }
        end += 1;
    }
    let rest = sql[end..].trim_start();
    format!("{}{rest}", &sql[..start])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coluna_gerada_fica_fora_mas_default_por_expressao_fica() {
        assert!(is_generated("VIRTUAL GENERATED"));
        assert!(is_generated("STORED GENERATED"));
        assert!(is_generated("PERSISTENT GENERATED"));
        assert!(!is_generated("DEFAULT_GENERATED"));
        assert!(!is_generated(
            "DEFAULT_GENERATED on update CURRENT_TIMESTAMP"
        ));
        assert!(!is_generated("auto_increment"));
    }

    #[test]
    fn binario_vai_em_hexa_e_o_resto_pelo_quote_do_servidor() {
        assert_eq!(value_expression("`nome`", "varchar"), "QUOTE(`nome`)");
        let blob = value_expression("`foto`", "LONGBLOB");
        assert!(blob.contains("HEX(`foto`)"));
        assert!(blob.contains("'NULL'"));
        assert!(blob.contains("''''''"), "binário vazio precisa virar ''");
    }

    #[test]
    fn tira_o_definer_em_todas_as_formas() {
        assert_eq!(
            strip_definer("CREATE ALGORITHM=UNDEFINED DEFINER=`root`@`%` SQL SECURITY DEFINER VIEW `v` AS select 1"),
            "CREATE ALGORITHM=UNDEFINED SQL SECURITY DEFINER VIEW `v` AS select 1"
        );
        assert_eq!(
            strip_definer("CREATE DEFINER=`app user`@`localhost` PROCEDURE `p`() BEGIN END"),
            "CREATE PROCEDURE `p`() BEGIN END"
        );
        assert_eq!(
            strip_definer("CREATE DEFINER=root@localhost TRIGGER t BEFORE INSERT ON x FOR EACH ROW SET @a = 1"),
            "CREATE TRIGGER t BEFORE INSERT ON x FOR EACH ROW SET @a = 1"
        );
        assert_eq!(
            strip_definer("CREATE TABLE t (a int)"),
            "CREATE TABLE t (a int)"
        );
    }
}
