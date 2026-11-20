//! O arquivo do backup em fluxo: escrever e ler sem carregá-lo inteiro.
//!
//! O formato é o envelope de sempre ([`super::service::BackupFile`]):
//!
//! ```json
//! {"formatVersion":1,"appVersion":"…","generatedAt":"…","scope":"full",
//!  "tables":{"users":[…],"sites":[…],…},
//!  "logTables":{"device_logs":[…]}}
//! ```
//!
//! O backup completo sai comprimido em gzip (`.json.gz`) e a leitura aceita
//! os dois — o `.json` da configuração continua valendo. Na escrita, cada
//! tabela é lida em fluxo e cada linha vira bytes na hora; na leitura, o JSON
//! é analisado em fluxo numa thread de bloqueio e as linhas chegam à gravação
//! em lotes. Nenhum dos lados guarda o arquivo na memória — o servidor aguenta
//! um banco de gigabytes.

use std::{fmt, io, pin::Pin};

use async_compression::tokio::bufread::{GzipDecoder, GzipEncoder};
use axum::body::Bytes;
use futures::Stream;
use sea_orm::{
    AccessMode, DatabaseBackend, DatabaseConnection, DatabaseTransaction, IsolationLevel,
    TransactionOptions, TransactionTrait,
};
use serde::{
    de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
    Serialize,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::{ReaderStream, StreamReader, SyncIoBridge};

use super::{
    catalog::{self, BackupScope, Rows, Store},
    service::FORMAT_VERSION,
};
use crate::{
    models::tables,
    services::shared::errors::{AppError, AppResult},
};

/// Quanto o escritor junta antes de entregar um pedaço ao corpo da resposta.
const CHUNK_BYTES: usize = 64 * 1024;
/// Linhas por lote entre a leitura e a gravação.
const ROWS_PER_BATCH: usize = 500;

/// Junta o JSON em pedaços e os entrega ao corpo da resposta.
///
/// O canal é limitado: se o navegador baixa devagar, a leitura do banco
/// espera — a memória do servidor não acumula o arquivo.
pub struct Chunker {
    buf: Vec<u8>,
    tx: mpsc::Sender<io::Result<Bytes>>,
    first: bool,
}

impl Chunker {
    fn new(tx: mpsc::Sender<io::Result<Bytes>>) -> Self {
        Self {
            buf: Vec::with_capacity(CHUNK_BYTES * 2),
            tx,
            first: true,
        }
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Abre um array: o próximo elemento vem sem vírgula.
    fn open_array(&mut self, key: &str) -> AppResult<()> {
        serde_json::to_writer(&mut self.buf, key).map_err(serialize_error)?;
        self.raw(b":[");
        self.first = true;
        Ok(())
    }

    /// Escreve um elemento do array aberto.
    ///
    /// # Errors
    ///
    /// Falha de serialização da linha.
    pub fn element<T: Serialize>(&mut self, value: &T) -> AppResult<()> {
        if !self.first {
            self.buf.push(b',');
        }
        self.first = false;
        serde_json::to_writer(&mut self.buf, value).map_err(serialize_error)
    }

    /// Entrega o que juntou quando passou do tamanho do pedaço.
    ///
    /// # Errors
    ///
    /// O navegador desistiu do download.
    pub async fn flush_if_full(&mut self) -> AppResult<()> {
        if self.buf.len() >= CHUNK_BYTES {
            self.flush().await?;
        }
        Ok(())
    }

    async fn flush(&mut self) -> AppResult<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let bytes = Bytes::from(std::mem::take(&mut self.buf));
        self.buf.reserve(CHUNK_BYTES * 2);
        self.tx
            .send(Ok(bytes))
            .await
            .map_err(|_| AppError::business_rule("O download do backup foi interrompido"))
    }
}

fn serialize_error(err: serde_json::Error) -> AppError {
    AppError::Internal(anyhow::anyhow!("serializar o backup: {err}"))
}

/// O cabeçalho do envelope, antes das tabelas.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Header<'a> {
    format_version: u32,
    app_version: &'a str,
    generated_at: String,
    scope: BackupScope,
}

/// O backup completo, em gzip, como fluxo de bytes para o corpo da resposta.
///
/// A escrita roda numa tarefa própria. Uma falha no meio vira erro **no
/// fluxo**: a conexão cai sem o fim do gzip, e o navegador recebe um download
/// interrompido em vez de um arquivo truncado com cara de inteiro.
pub fn export_full(
    db: DatabaseConnection,
    logs: Option<DatabaseConnection>,
    app_version: String,
) -> impl Stream<Item = io::Result<Bytes>> {
    let (tx, rx) = mpsc::channel(8);
    tokio::spawn(async move {
        let mut out = Chunker::new(tx.clone());
        if let Err(error) = write_full(&db, logs.as_ref(), &app_version, &mut out).await {
            tracing::warn!(%error, "backup completo interrompido");
            let _ = tx.send(Err(io::Error::other(error.to_string()))).await;
        }
    });
    let json = StreamReader::new(ReceiverStream::new(rx));
    ReaderStream::with_capacity(GzipEncoder::new(json), CHUNK_BYTES)
}

async fn write_full(
    db: &DatabaseConnection,
    logs: Option<&DatabaseConnection>,
    app_version: &str,
    out: &mut Chunker,
) -> AppResult<()> {
    let header = serde_json::to_vec(&Header {
        format_version: FORMAT_VERSION,
        app_version,
        generated_at: chrono::Utc::now().to_rfc3339(),
        scope: BackupScope::Full,
    })
    .map_err(serialize_error)?;
    // `{…}` → `{…,` para as tabelas entrarem no mesmo objeto.
    out.raw(&header[..header.len() - 1]);

    out.raw(b",\"tables\":{");
    write_store(db, Store::Main, out).await?;
    out.raw(b"}");
    if let Some(logs) = logs {
        out.raw(b",\"logTables\":{");
        write_store(logs, Store::Logs, out).await?;
        out.raw(b"}");
    }
    out.raw(b"}");
    out.flush().await
}

/// As tabelas de um banco, todas lidas da mesma fotografia: um histórico
/// gravado no meio do download não pode citar um monitor que ficou de fora.
async fn write_store(db: &DatabaseConnection, store: Store, out: &mut Chunker) -> AppResult<()> {
    let txn = begin_snapshot(db).await?;
    let existing = tables::existing_tables(&txn)
        .await
        .map_err(|err| AppError::Internal(anyhow::anyhow!("{err}")))?;
    let mut first = true;
    for table in catalog::full_tables(store) {
        if !existing.contains(table.name) {
            continue;
        }
        if !first {
            out.raw(b",");
        }
        first = false;
        out.open_array(table.name)?;
        table.dump(&txn, out).await?;
        out.raw(b"]");
        out.flush_if_full().await?;
    }
    txn.rollback().await?;
    Ok(())
}

/// Transação só de leitura com fotografia estável. No PostgreSQL, `REPEATABLE
/// READ`; no SQLite (WAL), a primeira leitura fixa a fotografia sozinha.
async fn begin_snapshot(db: &DatabaseConnection) -> AppResult<DatabaseTransaction> {
    let options = if db.get_database_backend() == DatabaseBackend::Postgres {
        TransactionOptions {
            isolation_level: Some(IsolationLevel::RepeatableRead),
            access_mode: Some(AccessMode::ReadOnly),
            ..TransactionOptions::default()
        }
    } else {
        TransactionOptions::default()
    };
    Ok(db.begin_with_options(options).await?)
}

// --- Leitura -------------------------------------------------------------------

/// O cabeçalho lido do arquivo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub format_version: Option<u32>,
    pub app_version: Option<String>,
    pub scope: BackupScope,
}

impl Meta {
    /// Recusa o arquivo antes de tocar no banco.
    ///
    /// # Errors
    ///
    /// Sem `formatVersion` (não é um backup do NetMonitor) ou numa versão que
    /// este sistema não lê.
    pub fn validate(&self) -> AppResult<()> {
        match self.format_version {
            Some(FORMAT_VERSION) => Ok(()),
            Some(other) => Err(AppError::validation(format!(
                "Arquivo de backup na versão {other} — este sistema lê a versão {FORMAT_VERSION}"
            ))),
            None => Err(AppError::validation(
                "O arquivo não é um backup do NetMonitor",
            )),
        }
    }
}

/// O que a leitura entrega, na ordem do arquivo.
#[derive(Debug)]
pub enum Event {
    /// O cabeçalho; vem antes de qualquer tabela.
    Meta(Meta),
    /// Um lote de linhas de uma tabela que a restauração grava.
    Rows {
        store: Store,
        table: String,
        rows: Rows,
    },
    /// A tabela acabou, com quantas linhas tinha.
    TableEnd {
        store: Store,
        table: String,
        rows: usize,
        accepted: bool,
    },
}

/// A leitura em andamento: os eventos e a tarefa que analisa o JSON.
pub struct Reading {
    pub events: mpsc::Receiver<Event>,
    parser: JoinHandle<Result<(), ParseFailure>>,
}

impl Reading {
    /// Espera a análise terminar e traduz a falha dela.
    ///
    /// # Errors
    ///
    /// O arquivo não é JSON, está truncado ou não tem a forma do envelope.
    pub async fn finish(self) -> AppResult<()> {
        drop(self.events);
        match self.parser.await {
            Ok(Ok(()) | Err(ParseFailure::Cancelled)) => Ok(()),
            Ok(Err(ParseFailure::Invalid(message))) => Err(AppError::validation(format!(
                "O arquivo não é um backup válido: {message}"
            ))),
            Err(join) => Err(AppError::Internal(anyhow::anyhow!(
                "leitura do backup: {join}"
            ))),
        }
    }
}

#[derive(Debug)]
enum ParseFailure {
    /// Quem consumia os eventos parou (erro na gravação, por exemplo).
    Cancelled,
    Invalid(String),
}

/// Marca, na mensagem do `serde`, que a parada veio de quem consome.
const CANCELLED: &str = "__netmonitor_backup_cancelled__";

/// Abre o arquivo (gzip ou JSON puro) e começa a analisá-lo.
///
/// Com `load`, as linhas das tabelas aceitas pelo escopo vêm nos eventos; sem,
/// só as contagens (a prévia).
///
/// # Errors
///
/// Falha de leitura do primeiro pedaço do corpo.
pub async fn read<R>(reader: R, load: bool) -> AppResult<Reading>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let reader = decompressed(reader).await?;
    let (tx, events) = mpsc::channel(4);
    let parser = tokio::task::spawn_blocking(move || {
        let sync = io::BufReader::with_capacity(256 * 1024, SyncIoBridge::new(reader));
        let mut deserializer = serde_json::Deserializer::from_reader(sync);
        let mut state = ParseState {
            tx,
            load,
            meta: Meta::default(),
            meta_sent: false,
        };
        let outcome = Envelope { state: &mut state }
            .deserialize(&mut deserializer)
            .and_then(|()| deserializer.end());
        outcome.map_err(|err| {
            if err.to_string().contains(CANCELLED) {
                ParseFailure::Cancelled
            } else {
                ParseFailure::Invalid(err.to_string())
            }
        })
    });
    Ok(Reading { events, parser })
}

type BoxedReader = Pin<Box<dyn AsyncRead + Send>>;

/// O gzip se denuncia nos dois primeiros bytes (`1f 8b`).
async fn decompressed<R>(reader: R) -> AppResult<BoxedReader>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let mut buffered = BufReader::with_capacity(256 * 1024, reader);
    let head = buffered
        .fill_buf()
        .await
        .map_err(|err| AppError::business_rule(format!("Falha ao ler o backup: {err}")))?;
    if head.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = GzipDecoder::new(buffered);
        decoder.multiple_members(true);
        Ok(Box::pin(decoder))
    } else {
        Ok(Box::pin(buffered))
    }
}

struct ParseState {
    tx: mpsc::Sender<Event>,
    load: bool,
    meta: Meta,
    meta_sent: bool,
}

impl ParseState {
    fn send<E: de::Error>(&self, event: Event) -> Result<(), E> {
        self.tx
            .blocking_send(event)
            .map_err(|_| E::custom(CANCELLED))
    }

    fn send_meta<E: de::Error>(&mut self) -> Result<(), E> {
        if self.meta_sent {
            return Ok(());
        }
        self.meta_sent = true;
        self.send(Event::Meta(self.meta.clone()))
    }
}

struct Envelope<'s> {
    state: &'s mut ParseState,
}

impl<'de> DeserializeSeed<'de> for Envelope<'_> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Envelope<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("o objeto do backup do NetMonitor")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "formatVersion" => self.state.meta.format_version = Some(map.next_value()?),
                "appVersion" => self.state.meta.app_version = Some(map.next_value()?),
                "scope" => self.state.meta.scope = map.next_value()?,
                "tables" | "logTables" => {
                    self.state.send_meta()?;
                    let store = if key == "tables" {
                        Store::Main
                    } else {
                        Store::Logs
                    };
                    map.next_value_seed(TablesSeed {
                        state: &mut *self.state,
                        store,
                    })?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        self.state.send_meta()
    }
}

struct TablesSeed<'s> {
    state: &'s mut ParseState,
    store: Store,
}

impl<'de> DeserializeSeed<'de> for TablesSeed<'_> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for TablesSeed<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("um mapa de tabelas")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(table) = map.next_key::<String>()? {
            let accepted = catalog::accepts(self.state.meta.scope, self.store, &table);
            let keep = accepted && self.state.load;
            let rows = map.next_value_seed(RowsSeed {
                state: &mut *self.state,
                store: self.store,
                table: &table,
                keep,
            })?;
            self.state.send(Event::TableEnd {
                store: self.store,
                table,
                rows,
                accepted,
            })?;
        }
        Ok(())
    }
}

struct RowsSeed<'s, 't> {
    state: &'s mut ParseState,
    store: Store,
    table: &'t str,
    /// Guarda as linhas para a gravação; sem, só conta.
    keep: bool,
}

impl<'de> DeserializeSeed<'de> for RowsSeed<'_, '_> {
    type Value = usize;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<usize, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for RowsSeed<'_, '_> {
    type Value = usize;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a lista de linhas da tabela '{}'", self.table)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<usize, A::Error> {
        let mut count = 0;
        if !self.keep {
            while seq.next_element::<IgnoredAny>()?.is_some() {
                count += 1;
            }
            return Ok(count);
        }
        let mut batch = Vec::with_capacity(ROWS_PER_BATCH);
        while let Some(row) = seq.next_element::<serde_json::Value>()? {
            batch.push(row);
            count += 1;
            if batch.len() == ROWS_PER_BATCH {
                self.state.send(Event::Rows {
                    store: self.store,
                    table: self.table.to_owned(),
                    rows: std::mem::replace(&mut batch, Vec::with_capacity(ROWS_PER_BATCH)),
                })?;
            }
        }
        if !batch.is_empty() {
            self.state.send(Event::Rows {
                store: self.store,
                table: self.table.to_owned(),
                rows: batch,
            })?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn events_of(bytes: Vec<u8>, load: bool) -> (Vec<Event>, AppResult<()>) {
        let mut reading = read(std::io::Cursor::new(bytes), load).await.unwrap();
        let mut events = Vec::new();
        while let Some(event) = reading.events.recv().await {
            events.push(event);
        }
        (events, reading.finish().await)
    }

    async fn gzip(bytes: &[u8]) -> Vec<u8> {
        use tokio::io::AsyncReadExt;
        let mut out = Vec::new();
        GzipEncoder::new(bytes).read_to_end(&mut out).await.unwrap();
        out
    }

    const FULL: &str = r#"{"formatVersion":1,"appVersion":"t","scope":"full",
        "tables":{"users":[{"id":1},{"id":2}],"sites":[],"tabela_do_futuro":[{"x":1}]},
        "logTables":{"device_logs":[{"id":9}]}}"#;

    #[tokio::test(flavor = "multi_thread")]
    async fn le_o_cabecalho_antes_das_tabelas_e_conta_as_linhas() {
        let (events, done) = events_of(FULL.as_bytes().to_vec(), false).await;
        done.unwrap();
        let Event::Meta(meta) = &events[0] else {
            panic!("o cabeçalho vem primeiro");
        };
        assert_eq!(meta.scope, BackupScope::Full);
        assert_eq!(meta.format_version, Some(FORMAT_VERSION));
        let ends: Vec<(String, usize, bool)> = events
            .iter()
            .filter_map(|event| match event {
                Event::TableEnd {
                    table,
                    rows,
                    accepted,
                    ..
                } => Some((table.clone(), *rows, *accepted)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ends,
            vec![
                ("users".into(), 2, true),
                ("sites".into(), 0, true),
                ("tabela_do_futuro".into(), 1, false),
                ("device_logs".into(), 1, true),
            ]
        );
        assert!(!events.iter().any(|e| matches!(e, Event::Rows { .. })));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gzip_e_carga_entregam_as_linhas_das_tabelas_aceitas() {
        let (events, done) = events_of(gzip(FULL.as_bytes()).await, true).await;
        done.unwrap();
        let rows: Vec<(String, usize)> = events
            .iter()
            .filter_map(|event| match event {
                Event::Rows { table, rows, .. } => Some((table.clone(), rows.len())),
                _ => None,
            })
            .collect();
        assert_eq!(rows, vec![("users".into(), 2), ("device_logs".into(), 1)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn arquivo_antigo_sem_escopo_e_de_configuracao() {
        let antigo = r#"{"formatVersion":1,"appVersion":"t","generatedAt":"x",
            "tables":{"sites":[{"id":1}],"users":[{"id":1}]}}"#;
        let (events, done) = events_of(antigo.as_bytes().to_vec(), true).await;
        done.unwrap();
        let Event::Meta(meta) = &events[0] else {
            panic!("cabeçalho");
        };
        assert_eq!(meta.scope, BackupScope::Config);
        // `users` não é da configuração: contada, nunca entregue.
        assert!(events.iter().any(|e| matches!(
            e,
            Event::TableEnd { table, accepted: false, .. } if table == "users"
        )));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn json_truncado_ou_tabela_que_nao_e_lista_e_recusado() {
        let (_, truncado) = events_of(
            br#"{"formatVersion":1,"tables":{"sites":[{"id":1}"#.to_vec(),
            true,
        )
        .await;
        assert!(truncado.is_err());
        let (_, nao_lista) = events_of(
            br#"{"formatVersion":1,"tables":{"sites":{"id":1}}}"#.to_vec(),
            false,
        )
        .await;
        assert!(nao_lista.is_err());
    }

    #[test]
    fn cabecalho_sem_versao_ou_de_outra_versao_e_recusado() {
        assert!(Meta::default().validate().is_err());
        let outra = Meta {
            format_version: Some(FORMAT_VERSION + 1),
            ..Meta::default()
        };
        assert!(outra.validate().is_err());
    }
}
