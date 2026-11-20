//! A restauração: apagar o que o arquivo substitui e gravar o que ele traz.
//!
//! Um caminho só para os dois escopos e as duas origens (o arquivo enviado em
//! fluxo e a cópia lida de um destino):
//!
//! * o cabeçalho é validado **antes** de qualquer escrita;
//! * cada banco grava numa transação (`begin_write`) aberta no início e
//!   confirmada no fim — um arquivo truncado no meio deixa tudo como estava;
//! * o completo substitui também os usuários. Sem um administrador ativo no
//!   arquivo, a restauração é recusada antes do `COMMIT`: ninguém conseguiria
//!   entrar depois.

use std::collections::HashSet;

use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, Statement,
};
use tokio::io::AsyncRead;

use super::{
    archive::{self, Event},
    catalog::{self, BackupScope, Rows, Store, KEPT_ON_FULL_RESTORE},
    service::{BACKED_UP_TABLES, DEPENDENT_HISTORY},
};
use crate::{
    models::{_entities::users, tables},
    services::{
        devices::system_device::{self, SystemDeviceService},
        monitoring::managed::ensure_system_health_monitor,
        shared::{
            errors::{AppError, AppResult},
            transaction::begin_write,
        },
    },
};

/// Quantas linhas cada tabela tinha (na prévia) ou recebeu (na restauração).
pub type TableCounts = Vec<(String, usize)>;

/// O que um arquivo contém, sem tocar no banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub scope: BackupScope,
    pub counts: TableCounts,
}

/// O resultado de uma restauração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub scope: BackupScope,
    pub counts: TableCounts,
    /// A conta de quem restaurou continua existindo e ativa. Falso depois de
    /// um completo vindo de outra instalação: é preciso entrar de novo, com
    /// uma conta do backup.
    pub session_kept: bool,
}

/// Lê o arquivo inteiro e conta as linhas do que a restauração gravaria.
///
/// # Errors
///
/// Arquivo que não é um backup, de outra versão ou malformado.
pub async fn inspect<R>(reader: R) -> AppResult<Preview>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let mut reading = archive::read(reader, false).await?;
    let mut scope = None;
    let mut counts = Vec::new();
    let mut outcome = Ok(());
    while let Some(event) = reading.events.recv().await {
        match event {
            Event::Meta(meta) => {
                if let Err(error) = meta.validate() {
                    outcome = Err(error);
                    break;
                }
                scope = Some(meta.scope);
            }
            Event::TableEnd {
                table,
                rows,
                accepted: true,
                ..
            } => counts.push((table, rows)),
            Event::TableEnd { .. } | Event::Rows { .. } => {}
        }
    }
    let parsed = reading.finish().await;
    outcome?;
    parsed?;
    Ok(Preview {
        scope: scope.ok_or_else(not_a_backup)?,
        counts,
    })
}

/// Restaura a partir do arquivo em fluxo.
///
/// `session_pid` é quem pediu: depois do completo, a resposta diz se a conta
/// dele ainda vale.
///
/// # Errors
///
/// Arquivo inválido, linha que não casa com o esquema, completo sem
/// administrador ou erro do banco. Em todos os casos os bancos ficam como
/// estavam.
pub async fn restore<R>(
    db: &DatabaseConnection,
    logs: Option<&DatabaseConnection>,
    reader: R,
    session_pid: Option<&str>,
) -> AppResult<Restored>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let mut reading = archive::read(reader, true).await?;
    let mut loader: Option<Loader> = None;
    let mut outcome = Ok(());
    while let Some(event) = reading.events.recv().await {
        let step = match event {
            Event::Meta(meta) => match meta.validate() {
                Ok(()) => Loader::begin(db, logs, meta.scope).await.map(|started| {
                    loader = Some(started);
                }),
                Err(error) => Err(error),
            },
            Event::Rows { store, table, rows } => match loader.as_mut() {
                Some(loader) => loader.rows(store, &table, rows).await,
                None => Err(not_a_backup()),
            },
            Event::TableEnd {
                store,
                table,
                accepted: true,
                ..
            } => match loader.as_mut() {
                Some(loader) => loader.table_end(store, &table).await,
                None => Err(not_a_backup()),
            },
            Event::TableEnd { .. } => Ok(()),
        };
        if let Err(error) = step {
            outcome = Err(error);
            break;
        }
    }
    let parsed = reading.finish().await;
    outcome?;
    parsed?;
    loader
        .ok_or_else(not_a_backup)?
        .finish(db, logs, session_pid)
        .await
}

fn not_a_backup() -> AppError {
    AppError::validation("O arquivo não é um backup do NetMonitor")
}

/// Uma tabela que chegou antes de outra que vem antes dela na ordem de
/// criação — arquivo reordenado por uma ferramenta (um `jq`, um editor, um
/// JSON regravado com as chaves em ordem alfabética). Espera, com as linhas,
/// até as anteriores terminarem, ou até o fim do arquivo.
struct Waiting {
    store: Store,
    table: String,
    rows: Rows,
    ended: bool,
}

/// A restauração em curso: as transações abertas e o que já entrou.
///
/// As tabelas entram na ordem de criação (pai antes de filho), seja qual for a
/// ordem do arquivo. O arquivo que o próprio sistema gera já vem nela e passa
/// em fluxo, sem nada retido; só a tabela adiantada espera na memória.
pub struct Loader {
    scope: BackupScope,
    main: DatabaseTransaction,
    logs: Option<DatabaseTransaction>,
    counts: TableCounts,
    current: usize,
    /// As tabelas já gravadas por inteiro.
    done: HashSet<(Store, String)>,
    waiting: Vec<Waiting>,
    /// `devices.parent_id` aponta para a própria tabela: as linhas esperam o
    /// fim dela para entrar com o pai antes do filho.
    held_devices: Rows,
}

impl Loader {
    /// Abre as transações e apaga o que o escopo substitui.
    ///
    /// # Errors
    ///
    /// Banco indisponível ou falha ao apagar.
    pub async fn begin(
        db: &DatabaseConnection,
        logs: Option<&DatabaseConnection>,
        scope: BackupScope,
    ) -> AppResult<Self> {
        let main = begin_write(db).await?;
        let logs = match scope {
            BackupScope::Config => {
                wipe_config(&main).await?;
                None
            }
            BackupScope::Full => {
                let wiped: Vec<&str> = tables::CREATION_ORDER
                    .iter()
                    .copied()
                    .filter(|table| !KEPT_ON_FULL_RESTORE.contains(table))
                    .collect();
                clear(&main, &wiped).await?;
                match logs {
                    Some(logs) => {
                        let txn = begin_write(logs).await?;
                        clear(&txn, &tables::LOG_TABLES).await?;
                        Some(txn)
                    }
                    None => None,
                }
            }
        };
        Ok(Self {
            scope,
            main,
            logs,
            counts: Vec::new(),
            current: 0,
            done: HashSet::new(),
            waiting: Vec::new(),
            held_devices: Vec::new(),
        })
    }

    /// As tabelas que este escopo grava neste banco, na ordem de criação.
    fn order(&self, store: Store) -> Vec<&'static str> {
        match self.scope {
            BackupScope::Config if store == Store::Main => BACKED_UP_TABLES.to_vec(),
            BackupScope::Config => Vec::new(),
            BackupScope::Full => catalog::full_tables(store).map(|io| io.name).collect(),
        }
    }

    /// Todas as tabelas que vêm antes desta já foram gravadas.
    fn ready(&self, store: Store, table: &str) -> bool {
        let order = self.order(store);
        let Some(at) = order.iter().position(|name| *name == table) else {
            return true;
        };
        order[..at]
            .iter()
            .all(|before| self.done.contains(&(store, (*before).to_owned())))
    }

    fn waiting_for(&mut self, store: Store, table: &str) -> Option<&mut Waiting> {
        self.waiting
            .iter_mut()
            .find(|waiting| waiting.store == store && waiting.table == table)
    }

    /// Grava um lote de linhas de uma tabela que o escopo aceita.
    ///
    /// # Errors
    ///
    /// Linha que não casa com o esquema ou erro do banco.
    pub async fn rows(&mut self, store: Store, table: &str, rows: Rows) -> AppResult<()> {
        if !catalog::accepts(self.scope, store, table) {
            return Ok(());
        }
        if let Some(waiting) = self.waiting_for(store, table) {
            waiting.rows.extend(rows);
            return Ok(());
        }
        if !self.ready(store, table) {
            self.waiting.push(Waiting {
                store,
                table: table.to_owned(),
                rows,
                ended: false,
            });
            return Ok(());
        }
        self.current += rows.len();
        if table == "devices" {
            self.held_devices.extend(rows);
            return Ok(());
        }
        self.insert(store, table, rows).await
    }

    /// Fecha a tabela: grava o que esperava e registra a contagem.
    ///
    /// # Errors
    ///
    /// Erro ao gravar as linhas guardadas.
    pub async fn table_end(&mut self, store: Store, table: &str) -> AppResult<()> {
        if !catalog::accepts(self.scope, store, table) {
            return Ok(());
        }
        if let Some(waiting) = self.waiting_for(store, table) {
            waiting.ended = true;
            return self.release().await;
        }
        if !self.ready(store, table) {
            // Tabela vazia adiantada: não tem linha, mas ainda conta.
            self.waiting.push(Waiting {
                store,
                table: table.to_owned(),
                rows: Vec::new(),
                ended: true,
            });
            return Ok(());
        }
        let count = std::mem::take(&mut self.current);
        self.close(store, table, count).await?;
        self.release().await
    }

    /// A tabela terminou: grava os dispositivos guardados e registra.
    async fn close(&mut self, store: Store, table: &str, count: usize) -> AppResult<()> {
        if table == "devices" && !self.held_devices.is_empty() {
            let held = parents_first(std::mem::take(&mut self.held_devices));
            self.insert(store, table, held).await?;
        }
        self.counts.push((table.to_owned(), count));
        self.done.insert((store, table.to_owned()));
        Ok(())
    }

    /// Grava as tabelas que esperavam e cujas anteriores já entraram.
    async fn release(&mut self) -> AppResult<()> {
        while let Some(at) = self
            .waiting
            .iter()
            .position(|waiting| waiting.ended && self.ready(waiting.store, &waiting.table))
        {
            let waiting = self.waiting.remove(at);
            self.flush(waiting).await?;
        }
        Ok(())
    }

    async fn flush(&mut self, waiting: Waiting) -> AppResult<()> {
        let count = waiting.rows.len();
        if waiting.table == "devices" {
            self.held_devices = waiting.rows;
        } else if !waiting.rows.is_empty() {
            self.insert(waiting.store, &waiting.table, waiting.rows)
                .await?;
        }
        self.close(waiting.store, &waiting.table, count).await
    }

    async fn insert(&self, store: Store, table: &str, rows: Rows) -> AppResult<()> {
        let txn = match store {
            Store::Main => &self.main,
            // Sem banco de logs nesta instalação (syslog desligado): os logs
            // do arquivo ficam de fora, o resto volta.
            Store::Logs => match &self.logs {
                Some(txn) => txn,
                None => return Ok(()),
            },
        };
        let Some(io) = catalog::find(store, table) else {
            return Ok(());
        };
        io.insert(txn, rows).await
    }

    /// Confere, confirma e refaz o que o sistema precisa para rodar.
    ///
    /// # Errors
    ///
    /// Completo sem administrador ativo, ou erro do banco ao confirmar.
    pub async fn finish(
        mut self,
        db: &DatabaseConnection,
        logs: Option<&DatabaseConnection>,
        session_pid: Option<&str>,
    ) -> AppResult<Restored> {
        // Fim do arquivo: quem esperava uma tabela que não veio (arquivo de uma
        // versão com menos tabelas) entra agora, na ordem de criação.
        let mut left = std::mem::take(&mut self.waiting);
        let position = |loader: &Self, waiting: &Waiting| {
            let store = usize::from(waiting.store == Store::Logs);
            let at = loader
                .order(waiting.store)
                .iter()
                .position(|name| *name == waiting.table)
                .unwrap_or(usize::MAX);
            (store, at)
        };
        left.sort_by_key(|waiting| position(&self, waiting));
        for waiting in left {
            self.flush(waiting).await?;
        }
        match self.scope {
            BackupScope::Config => realign_sequences(&self.main, &BACKED_UP_TABLES).await?,
            BackupScope::Full => {
                let restored: Vec<&str> =
                    catalog::full_tables(Store::Main).map(|t| t.name).collect();
                realign_sequences(&self.main, &restored).await?;
                let admins = users::Entity::find()
                    .filter(users::Column::Role.eq("admin"))
                    .filter(users::Column::Active.eq(true))
                    .count(&self.main)
                    .await?;
                if admins == 0 {
                    return Err(AppError::validation(
                        "O backup não tem nenhum administrador ativo — depois dele ninguém \
                         conseguiria entrar. Nada foi alterado.",
                    ));
                }
                if let Some(logs) = &self.logs {
                    realign_sequences(logs, &["device_logs"]).await?;
                }
            }
        }

        self.main.commit().await?;
        if let Some(logs) = self.logs {
            logs.commit().await?;
        }
        after_restore(db, logs).await;

        let session_kept = match session_pid {
            Some(pid) => session_still_valid(db, pid).await,
            None => true,
        };
        Ok(Restored {
            scope: self.scope,
            counts: self.counts,
            session_kept,
        })
    }
}

/// O escopo de configuração: a configuração e o histórico que cita ids dela.
///
/// Elas não estão no arquivo, mas apontam para `devices`/`monitors`/`probes`,
/// que são substituídos — deixá-las produziria histórico pendurado em id que
/// passou a ser de outro equipamento.
async fn wipe_config(txn: &DatabaseTransaction) -> AppResult<()> {
    let existing = existing(txn).await?;
    let backend = txn.get_database_backend();
    // Plugin exclusivo de um equipamento some com ele. Os de modelo ficam: não
    // pertencem a nenhum id que a restauração troca. O ajuste de um
    // equipamento idem; a configuração da frota fica.
    for table in ["plugins", "plugin_settings"] {
        if existing.contains(table) {
            txn.execute_raw(Statement::from_string(
                backend,
                format!("DELETE FROM \"{table}\" WHERE \"device_id\" IS NOT NULL"),
            ))
            .await?;
        }
    }
    let doomed: Vec<&str> = DEPENDENT_HISTORY
        .iter()
        .copied()
        .chain(BACKED_UP_TABLES.iter().rev().copied())
        .collect();
    delete_in_order(txn, &doomed, &existing).await
}

/// Esvazia as tabelas. No PostgreSQL, um `TRUNCATE` só (rápido, e a ordem das
/// FKs não importa porque todas vão juntas); no SQLite, `DELETE` de filho para
/// pai.
async fn clear(txn: &DatabaseTransaction, names: &[&str]) -> AppResult<()> {
    let existing = existing(txn).await?;
    let present: Vec<&str> = names
        .iter()
        .copied()
        .filter(|table| existing.contains(*table))
        .collect();
    if present.is_empty() {
        return Ok(());
    }
    if txn.get_database_backend() == DatabaseBackend::Postgres {
        let list = present
            .iter()
            .map(|table| format!("\"{table}\""))
            .collect::<Vec<_>>()
            .join(", ");
        txn.execute_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("TRUNCATE TABLE {list}"),
        ))
        .await?;
        return Ok(());
    }
    let children_first: Vec<&str> = present.into_iter().rev().collect();
    delete_in_order(txn, &children_first, &existing).await
}

async fn delete_in_order(
    txn: &DatabaseTransaction,
    order: &[&str],
    existing: &HashSet<String>,
) -> AppResult<()> {
    let backend = txn.get_database_backend();
    for table in order {
        if existing.contains(*table) {
            txn.execute_raw(Statement::from_string(
                backend,
                format!("DELETE FROM \"{table}\""),
            ))
            .await?;
        }
    }
    Ok(())
}

async fn existing(txn: &DatabaseTransaction) -> AppResult<HashSet<String>> {
    tables::existing_tables(txn)
        .await
        .map_err(|err| AppError::Internal(anyhow::anyhow!("{err}")))
}

/// Realinha as sequências de `id` no PostgreSQL depois de uma carga com ids
/// explícitos.
///
/// O `bigserial` não sabe que alguém inseriu id 42 à mão: a sequência continua
/// no valor de antes e o próximo cadastro estouraria com violação de chave
/// primária. `setval` com o maior id existente conserta isso. No SQLite não há
/// o que fazer — o `rowid` seguinte é derivado do maior valor da tabela.
async fn realign_sequences(txn: &DatabaseTransaction, names: &[&str]) -> AppResult<()> {
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }
    let with_id: HashSet<String> = txn
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT table_name::text AS name FROM information_schema.columns \
             WHERE table_schema = current_schema() AND column_name = 'id'",
        ))
        .await?
        .into_iter()
        .filter_map(|row| row.try_get::<String>("", "name").ok())
        .collect();
    for table in names.iter().filter(|table| with_id.contains(**table)) {
        // `is_called = false` faz o próximo `nextval` devolver o próprio valor
        // passado, o que dá o comportamento certo na tabela vazia (começa em
        // 1). Tabela de `id` sem sequência devolve `NULL` e nada muda.
        let sql = format!(
            "SELECT setval(pg_get_serial_sequence('{table}', 'id'), \
             COALESCE((SELECT MAX(id) FROM \"{table}\"), 0) + 1, false)"
        );
        txn.execute_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
            .await?;
    }
    Ok(())
}

/// Ordena os dispositivos para o pai entrar antes do filho. Pai que não está
/// no arquivo não segura a linha: a FK recusa e a restauração inteira volta.
fn parents_first(rows: Rows) -> Rows {
    let id_of = |row: &serde_json::Value| row.get("id").and_then(serde_json::Value::as_i64);
    let ids: HashSet<i64> = rows.iter().filter_map(id_of).collect();
    let mut placed = HashSet::with_capacity(rows.len());
    let mut ordered = Vec::with_capacity(rows.len());
    let mut pending = rows;
    loop {
        let before = pending.len();
        let mut waiting = Vec::new();
        for row in pending {
            let parent = row.get("parent_id").and_then(serde_json::Value::as_i64);
            if parent.is_none_or(|p| placed.contains(&p) || !ids.contains(&p)) {
                if let Some(id) = id_of(&row) {
                    placed.insert(id);
                }
                ordered.push(row);
            } else {
                waiting.push(row);
            }
        }
        if waiting.is_empty() || waiting.len() == before {
            ordered.extend(waiting);
            return ordered;
        }
        pending = waiting;
    }
}

/// O que roda depois do `COMMIT`, nos dois escopos.
async fn after_restore(db: &DatabaseConnection, logs: Option<&DatabaseConnection>) {
    // `DELETE` devolve páginas ao freelist, não ao filesystem. A compactação
    // é best effort: a restauração já foi confirmada.
    compact_sqlite(db).await;
    if let Some(logs) = logs {
        compact_sqlite(logs).await;
    }
    // Os ids voltaram os do arquivo: a linha de base guardada do monitor 1 de
    // antes não pode valer para o monitor 1 de agora.
    crate::services::alerts::baseline::clear_cache();

    // O ID cacheado do dispositivo do sistema passaria a apontar para outro
    // equipamento, e os logs internos iriam parar num roteador qualquer.
    system_device::resolver::invalidate();
    match SystemDeviceService::new(db).ensure().await {
        Ok(device) => {
            // O arquivo pode ser anterior à coleta de saúde, ou tê-la perdido
            // junto com o dispositivo.
            if let Err(error) = ensure_system_health_monitor(db, device.id).await {
                tracing::warn!(%error, "não foi possível reprovisionar a coleta de saúde após restore");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "não foi possível regarantir o dispositivo do sistema após restore");
        }
    }
}

async fn compact_sqlite(db: &DatabaseConnection) {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return;
    }
    for statement in [
        "PRAGMA wal_checkpoint(TRUNCATE);",
        "VACUUM;",
        "PRAGMA optimize;",
    ] {
        if let Err(error) = db.execute_unprepared(statement).await {
            tracing::warn!(%error, statement, "não foi possível compactar o SQLite após restore");
        }
    }
}

async fn session_still_valid(db: &DatabaseConnection, pid: &str) -> bool {
    let Ok(pid) = uuid::Uuid::parse_str(pid) else {
        return false;
    };
    users::Entity::find()
        .filter(users::Column::Pid.eq(pid))
        .filter(users::Column::Active.eq(true))
        .count(db)
        .await
        .is_ok_and(|found| found > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pai_entra_antes_do_filho() {
        let ordem: Vec<i64> = parents_first(vec![
            json!({"id": 1, "parent_id": 3}),
            json!({"id": 2, "parent_id": null}),
            json!({"id": 3, "parent_id": 2}),
            json!({"id": 4, "parent_id": 99}),
        ])
        .iter()
        .map(|row| row["id"].as_i64().unwrap())
        .collect();
        assert_eq!(ordem, vec![2, 3, 4, 1]);
    }
}
