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
    archive::{self, Event, FULL_EXPORT_SCOPE},
    catalog::{self, BackupScope, Rows, Store, KEPT_ON_FULL_RESTORE},
    copies::RUN_SCOPE,
    plan::PLAN_ID,
    service::{BACKED_UP_TABLES, DEPENDENT_HISTORY},
};
use crate::{
    models::{_entities::users, tables},
    services::{
        devices::system_device::{self, SystemDeviceService},
        monitoring::managed::ensure_system_health_monitor,
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
            transaction::begin_write,
        },
        syslog,
    },
};

/// Teto de bytes retidos em memória para tabelas fora de ordem (JSON reordenado)
/// no backup completo, que pode ter gigabytes de histórico.
const MAX_WAITING_BYTES: usize = 64 * 1024 * 1024;

/// O mesmo teto no escopo de configuração. O arquivo inteiro já cabe na
/// memória por construção — a cópia lida de um destino é carregada inteira
/// até 256 MB (`copies::read`) — e um JSON de configuração reordenado por uma
/// ferramenta (`jq`, um editor) é exatamente o caso que a espera existe para
/// aceitar.
const MAX_WAITING_BYTES_CONFIG: usize = 256 * 1024 * 1024;

/// As travas que a restauração segura enquanto roda — e, depois do `COMMIT`,
/// até a compactação do SQLite terminar.
///
/// Duas, e não uma: a de [`RUN_SCOPE`] a exclui do envio de cópia (o
/// agendado e o "Fazer backup agora"), e a de [`FULL_EXPORT_SCOPE`] a exclui
/// do download do backup completo — que, lendo no meio de um `TRUNCATE`,
/// geraria um arquivo com metade dos bancos.
pub struct RestoreGuard {
    _copies: RunGuard,
    _full_export: RunGuard,
}

impl RestoreGuard {
    const BUSY: &'static str = "Já há um backup ou restauração do NetMonitor em andamento";

    /// # Errors
    ///
    /// `409` com outra restauração, cópia ou backup completo em andamento.
    pub fn acquire() -> AppResult<Self> {
        Ok(Self {
            _copies: RunGuard::acquire(RUN_SCOPE, PLAN_ID, Self::BUSY)?,
            _full_export: RunGuard::acquire(FULL_EXPORT_SCOPE, PLAN_ID, Self::BUSY)?,
        })
    }
}

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
    let guard = RestoreGuard::acquire()?;
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
        .finish(db, logs, session_pid, guard)
        .await
}

/// Quanto o escopo deixa esperar na memória por uma tabela adiantada.
const fn waiting_cap(scope: BackupScope) -> usize {
    match scope {
        BackupScope::Config => MAX_WAITING_BYTES_CONFIG,
        BackupScope::Full => MAX_WAITING_BYTES,
    }
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
    bytes: usize,
    ended: bool,
}

enum LogsTarget {
    None,
    /// `device_logs` mora no mesmo banco que o principal: reaproveita `&self.main`
    /// para evitar deadlock de duas transações de escrita no mesmo banco.
    SharedMain,
    /// Banco separado (`logs.sqlite` ou `SYSLOG_DB_URL` distinta).
    Separate(DatabaseTransaction),
}

fn store_index(store: Store) -> usize {
    usize::from(store == Store::Logs)
}

fn json_size_hint(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) => 8,
        serde_json::Value::Number(_) => 16,
        serde_json::Value::String(s) => 24 + s.len(),
        serde_json::Value::Array(items) => 24 + items.iter().map(json_size_hint).sum::<usize>(),
        serde_json::Value::Object(map) => {
            32 + map
                .iter()
                .map(|(k, v)| 24 + k.len() + json_size_hint(v))
                .sum::<usize>()
        }
    }
}

fn estimate_rows_bytes(rows: &[serde_json::Value]) -> usize {
    rows.iter().map(json_size_hint).sum()
}

async fn sqlite_db_file(db: &DatabaseConnection) -> Option<String> {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return None;
    }
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA database_list;",
        ))
        .await
        .ok()??;
    row.try_get::<String>("", "file")
        .ok()
        .map(|file| file.trim().to_owned())
        .filter(|file| !file.is_empty())
}

async fn logs_shares_main(
    db: &DatabaseConnection,
    logs: &DatabaseConnection,
    main_existing: &HashSet<String>,
) -> bool {
    if db.get_database_backend() != logs.get_database_backend() {
        return false;
    }
    if db.get_database_backend() == DatabaseBackend::Postgres
        && !syslog::db::is_separate("postgres://local")
    {
        return true;
    }
    if std::env::var("DATABASE_URL")
        .ok()
        .is_some_and(|url| !syslog::db::is_separate(&url))
        && main_existing.contains("device_logs")
    {
        return true;
    }
    if db.get_database_backend() == DatabaseBackend::Sqlite && main_existing.contains("device_logs")
    {
        if let (Some(main_file), Some(logs_file)) =
            (sqlite_db_file(db).await, sqlite_db_file(logs).await)
        {
            return main_file == logs_file;
        }
    }
    false
}

/// A restauração em curso: as transações abertas e o que já entrou.
///
/// As tabelas entram na ordem de criação (pai antes de filho), seja qual for a
/// ordem do arquivo. O arquivo que o próprio sistema gera já vem nela e passa
/// em fluxo, sem nada retido; só a tabela adiantada espera na memória.
pub struct Loader {
    scope: BackupScope,
    main: DatabaseTransaction,
    logs: LogsTarget,
    counts: TableCounts,
    current: usize,
    /// As tabelas já gravadas por inteiro.
    done: HashSet<(Store, String)>,
    /// Maior posição já iniciada em fluxo por banco, para permitir streaming
    /// monotônico mesmo quando alguma tabela anterior foi omitida no arquivo.
    highest_streamed: [Option<usize>; 2],
    out_of_order: [bool; 2],
    waiting: Vec<Waiting>,
    waiting_bytes: usize,
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
                LogsTarget::None
            }
            BackupScope::Full => {
                let main_existing = existing(&main).await?;
                let wiped: Vec<&str> = tables::CREATION_ORDER
                    .iter()
                    .copied()
                    .filter(|table| !KEPT_ON_FULL_RESTORE.contains(table))
                    .collect();
                clear(&main, &wiped).await?;
                match logs {
                    Some(logs_db) => {
                        if logs_shares_main(db, logs_db, &main_existing).await {
                            clear(&main, &tables::LOG_TABLES).await?;
                            LogsTarget::SharedMain
                        } else {
                            let txn = begin_write(logs_db).await?;
                            clear(&txn, &tables::LOG_TABLES).await?;
                            LogsTarget::Separate(txn)
                        }
                    }
                    None => LogsTarget::None,
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
            highest_streamed: [None, None],
            out_of_order: [false, false],
            waiting: Vec::new(),
            waiting_bytes: 0,
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

    fn position_of(&self, store: Store, table: &str) -> Option<usize> {
        self.order(store).iter().position(|name| *name == table)
    }

    /// A tabela pode entrar em fluxo imediatamente sem violar a ordem de
    /// criação.
    ///
    /// Aceita avanço monotônico quando tabelas anteriores não vieram no arquivo
    /// (backup de versão anterior ou tabela inexistente pulada na exportação),
    /// evitando reter gigabytes em `self.waiting`.
    fn ready(&self, store: Store, table: &str) -> bool {
        let order = self.order(store);
        let Some(at) = order.iter().position(|name| *name == table) else {
            return true;
        };
        if order[..at]
            .iter()
            .all(|before| self.done.contains(&(store, (*before).to_owned())))
        {
            return true;
        }
        // `device_logs` não tem FK para `log_templates`: um backup sem
        // `log_templates` pode gravar `device_logs` direto em fluxo.
        if store == Store::Logs {
            return true;
        }
        let idx = store_index(store);
        let has_waiting = self.waiting.iter().any(|w| w.store == store);
        !self.out_of_order[idx]
            && !has_waiting
            && self.highest_streamed[idx].is_some_and(|prev| {
                at > prev || (at == prev && !self.done.contains(&(store, table.to_owned())))
            })
    }

    fn record_streamed(&mut self, store: Store, table: &str) {
        if let Some(at) = self.position_of(store, table) {
            let idx = store_index(store);
            self.highest_streamed[idx] = Some(self.highest_streamed[idx].map_or(at, |p| p.max(at)));
        }
    }

    fn waiting_for(&mut self, store: Store, table: &str) -> Option<&mut Waiting> {
        self.waiting
            .iter_mut()
            .find(|waiting| waiting.store == store && waiting.table == table)
    }

    fn charge_waiting(&mut self, added_bytes: usize) -> AppResult<()> {
        self.waiting_bytes = self.waiting_bytes.saturating_add(added_bytes);
        let cap = waiting_cap(self.scope);
        if self.waiting_bytes > cap {
            return Err(AppError::validation(format!(
                "O arquivo de backup está fora de ordem e excede o limite de memória ({} MB) \
                 para reordenação. Gere o backup novamente sem reordenar as chaves.",
                cap / (1024 * 1024)
            )));
        }
        Ok(())
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
        if self.waiting_for(store, table).is_some() {
            let bytes = estimate_rows_bytes(&rows);
            self.charge_waiting(bytes)?;
            if let Some(waiting) = self.waiting_for(store, table) {
                waiting.rows.extend(rows);
                waiting.bytes = waiting.bytes.saturating_add(bytes);
            }
            return Ok(());
        }
        if !self.ready(store, table) {
            self.out_of_order[store_index(store)] = true;
            let bytes = estimate_rows_bytes(&rows);
            self.charge_waiting(bytes)?;
            self.waiting.push(Waiting {
                store,
                table: table.to_owned(),
                rows,
                bytes,
                ended: false,
            });
            return Ok(());
        }
        self.record_streamed(store, table);
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
            self.out_of_order[store_index(store)] = true;
            // Tabela vazia adiantada: não tem linha, mas ainda conta.
            self.waiting.push(Waiting {
                store,
                table: table.to_owned(),
                rows: Vec::new(),
                bytes: 0,
                ended: true,
            });
            return Ok(());
        }
        self.record_streamed(store, table);
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
            self.waiting_bytes = self.waiting_bytes.saturating_sub(waiting.bytes);
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
                LogsTarget::SharedMain => &self.main,
                LogsTarget::Separate(txn) => txn,
                LogsTarget::None => return Ok(()),
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
        guard: RestoreGuard,
    ) -> AppResult<Restored> {
        // Fim do arquivo: quem esperava uma tabela que não veio (arquivo de uma
        // versão com menos tabelas) entra agora, na ordem de criação.
        let mut left = std::mem::take(&mut self.waiting);
        self.waiting_bytes = 0;
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
                match &self.logs {
                    LogsTarget::SharedMain => {
                        realign_sequences(&self.main, &["device_logs"]).await?;
                    }
                    LogsTarget::Separate(logs_txn) => {
                        realign_sequences(logs_txn, &["device_logs"]).await?;
                    }
                    LogsTarget::None => {}
                }
            }
        }

        let compact_logs = matches!(self.logs, LogsTarget::Separate(_));
        self.main.commit().await?;
        if let LogsTarget::Separate(logs_txn) = self.logs {
            if let Err(error) = logs_txn.commit().await {
                tracing::error!(
                    %error,
                    "restauração parcial: banco principal confirmado, mas o banco de logs falhou ao confirmar"
                );
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Banco principal restaurado, mas o banco de logs falhou ao confirmar: {error}"
                )));
            }
        }
        after_restore(db, if compact_logs { logs } else { None }, guard).await;

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
///
/// `guard` vai junto para a compactação: um `VACUUM` do SQLite reescreve o
/// arquivo inteiro, e um backup ou outra restauração começando no meio dele
/// esperaria a trava do banco até estourar o tempo.
async fn after_restore(
    db: &DatabaseConnection,
    logs: Option<&DatabaseConnection>,
    guard: RestoreGuard,
) {
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

    // `DELETE` devolve páginas ao freelist, não ao filesystem. A compactação
    // roda em segundo plano para não bloquear a resposta HTTP (e evitar timeout
    // em bancos SQLite grandes): a restauração já foi confirmada.
    let main_db = db.clone();
    let logs_db = logs.cloned();
    tokio::spawn(async move {
        let _guard = guard;
        compact_sqlite(&main_db).await;
        if let Some(logs_db) = logs_db {
            compact_sqlite(&logs_db).await;
        }
    });
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
    use crate::models::_entities::sites;
    use sea_orm::{ActiveValue::Set, PaginatorTrait};
    use serde_json::json;
    use serial_test::serial;

    async fn banco() -> DatabaseConnection {
        use crate::test_schema::{MigratorTrait, TestMigrator as Migrator};
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

    async fn semeia_sites(db: &DatabaseConnection, quantos: usize) {
        let agora: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
        let linhas: Vec<sites::ActiveModel> = (0..quantos)
            .map(|n| sites::ActiveModel {
                name: Set(format!("Site {n} {}", "x".repeat(200))),
                active: Set(true),
                created_at: Set(agora),
                updated_at: Set(agora),
                ..Default::default()
            })
            .collect();
        for bloco in linhas.chunks(500) {
            sites::Entity::insert_many(bloco.to_vec())
                .exec(db)
                .await
                .expect("sites");
        }
    }

    #[tokio::test]
    #[serial]
    async fn o_proprio_export_volta_em_fluxo_e_reordenado_tambem() {
        let db = banco().await;
        semeia_sites(&db, 3_000).await;
        let arquivo = super::super::service::export(&db, "teste".into())
            .await
            .expect("export");
        let bytes = serde_json::to_vec(&arquivo).expect("serializa");

        let restaurado = restore(&db, None, std::io::Cursor::new(bytes.clone()), None)
            .await
            .expect("o export do próprio sistema tem de voltar");
        assert_eq!(restaurado.scope, BackupScope::Config);
        assert_eq!(sites::Entity::find().count(&db).await.unwrap(), 3_000);

        // A compactação em segundo plano segura a vez até terminar.
        for _ in 0..200 {
            if !RunGuard::is_running(RUN_SCOPE, PLAN_ID) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // O mesmo arquivo regravado com as chaves em ordem alfabética (um `jq`,
        // um editor) também volta: no escopo de configuração a espera cabe.
        let valor: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let alfabetico = serde_json::to_vec(&valor).unwrap();
        restore(&db, None, std::io::Cursor::new(alfabetico), None)
            .await
            .expect("arquivo reordenado");
        assert_eq!(sites::Entity::find().count(&db).await.unwrap(), 3_000);
    }

    #[test]
    #[serial]
    fn restauracao_e_backup_completo_se_excluem() {
        let restaurando = RestoreGuard::acquire().expect("primeira");
        assert!(RunGuard::acquire(FULL_EXPORT_SCOPE, PLAN_ID, "ocupado").is_err());
        assert!(RunGuard::acquire(RUN_SCOPE, PLAN_ID, "ocupado").is_err());
        assert!(RestoreGuard::acquire().is_err());
        drop(restaurando);

        let exportando = RunGuard::acquire(FULL_EXPORT_SCOPE, PLAN_ID, "ocupado").unwrap();
        assert!(RestoreGuard::acquire().is_err());
        assert!(
            !RunGuard::is_running(RUN_SCOPE, PLAN_ID),
            "a tentativa recusada não pode deixar a outra trava presa"
        );
        drop(exportando);
        assert!(RestoreGuard::acquire().is_ok());
    }

    #[test]
    fn o_teto_de_espera_da_configuracao_cobre_o_arquivo_inteiro() {
        assert_eq!(waiting_cap(BackupScope::Full), MAX_WAITING_BYTES);
        assert!(waiting_cap(BackupScope::Config) >= 256 * 1024 * 1024);
    }

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
