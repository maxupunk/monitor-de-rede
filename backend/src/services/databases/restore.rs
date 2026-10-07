//! O laço que aplica um dump, comum aos dois SGBDs.
//!
//! O arquivo é lido em fluxo (gzip → linhas), dividido em comandos pelo
//! [`StatementSplitter`] e entregue a uma [`RestoreSession`], que é o único
//! pedaço que conhece o servidor. Os blocos `COPY ... FROM stdin` do
//! PostgreSQL vão direto para o `COPY` do servidor, sem passar por `INSERT`.
//!
//! A soma do SQL é calculada enquanto ele é lido e conferida **antes** de o
//! driver confirmar a transação: um arquivo corrompido no armazenamento não
//! chega a substituir o banco.

use std::pin::Pin;

use async_compression::tokio::bufread::GzipDecoder;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};

use super::{
    sql::splitter::{is_copy_from_stdin, StatementSplitter},
    DatabaseError, EngineFamily, Progress,
};
use crate::services::storage::ObjectReader;

/// Totais de uma restauração.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreStats {
    pub statements: u64,
    pub rows: u64,
}

/// Linhas de um dump compactado, com a soma calculada no caminho.
pub struct LineReader {
    inner: Pin<Box<dyn AsyncBufRead + Send>>,
    hasher: Sha256,
    bytes: u64,
    expected_checksum: Option<String>,
    buffer: String,
}

impl LineReader {
    /// Lê um `.sql.gz`. Com `expected_checksum`, [`verify`](Self::verify)
    /// confere a soma no fim.
    #[must_use]
    pub fn gzip(reader: ObjectReader, expected_checksum: Option<String>) -> Self {
        let decoder = GzipDecoder::new(BufReader::with_capacity(256 * 1024, reader));
        Self::plain(Box::pin(BufReader::new(decoder)), expected_checksum)
    }

    /// Lê SQL já descompactado.
    #[must_use]
    pub fn plain(
        inner: Pin<Box<dyn AsyncBufRead + Send>>,
        expected_checksum: Option<String>,
    ) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
            expected_checksum,
            buffer: String::new(),
        }
    }

    /// A próxima linha, sem o `\n`; `None` no fim do arquivo.
    ///
    /// # Errors
    ///
    /// Falha de leitura, gzip corrompido ou texto que não é UTF-8.
    pub async fn next_line(&mut self) -> Result<Option<String>, DatabaseError> {
        self.buffer.clear();
        let read = self
            .inner
            .read_line(&mut self.buffer)
            .await
            .map_err(DatabaseError::io)?;
        if read == 0 {
            return Ok(None);
        }
        self.hasher.update(self.buffer.as_bytes());
        self.bytes += read as u64;
        let line = self.buffer.strip_suffix('\n').unwrap_or(&self.buffer);
        Ok(Some(line.strip_suffix('\r').unwrap_or(line).to_string()))
    }

    /// Bytes lidos até agora (descompactados).
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Confere a soma do que foi lido.
    ///
    /// # Errors
    ///
    /// [`DatabaseError::Checksum`] quando não confere.
    pub fn verify(&self) -> Result<(), DatabaseError> {
        match &self.expected_checksum {
            Some(expected) if *expected != hex::encode(self.hasher.clone().finalize()) => {
                Err(DatabaseError::Checksum)
            }
            _ => Ok(()),
        }
    }
}

/// O servidor do outro lado da restauração.
#[async_trait]
pub trait RestoreSession: Send {
    /// Executa um comando; devolve as linhas afetadas.
    async fn execute(&mut self, statement: &str) -> Result<u64, DatabaseError>;

    /// `COPY ... FROM stdin`: consome as linhas de `input` até `\.`.
    async fn copy_from(
        &mut self,
        _statement: &str,
        _input: &mut LineReader,
    ) -> Result<u64, DatabaseError> {
        Err(DatabaseError::Unsupported(
            "COPY FROM stdin só existe no PostgreSQL".to_string(),
        ))
    }
}

/// De quantas em quantas linhas o andamento é publicado.
const PROGRESS_EVERY_LINES: u64 = 2_000;

/// Aplica o script inteiro e confere a soma.
///
/// # Errors
///
/// O primeiro comando que falhar, com o número dele e o começo do texto — é o
/// que permite ao operador achar o problema num dump de milhares de linhas.
pub async fn apply_script(
    session: &mut dyn RestoreSession,
    family: EngineFamily,
    input: &mut LineReader,
    progress: &dyn Progress,
) -> Result<RestoreStats, DatabaseError> {
    let mut splitter = StatementSplitter::new(family);
    let mut stats = RestoreStats::default();
    let mut lines = 0_u64;

    while let Some(line) = input.next_line().await? {
        lines += 1;
        for statement in splitter.push_line(&line) {
            run_one(session, family, &statement, input, progress, &mut stats).await?;
        }
        if lines.is_multiple_of(PROGRESS_EVERY_LINES) {
            progress.bytes(input.bytes());
        }
    }
    if let Some(statement) = splitter.finish() {
        run_one(session, family, &statement, input, progress, &mut stats).await?;
    }
    progress.bytes(input.bytes());
    input.verify()?;
    Ok(stats)
}

async fn run_one(
    session: &mut dyn RestoreSession,
    family: EngineFamily,
    statement: &str,
    input: &mut LineReader,
    progress: &dyn Progress,
    stats: &mut RestoreStats,
) -> Result<(), DatabaseError> {
    stats.statements += 1;
    let number = stats.statements;
    let fail = |error: DatabaseError| {
        DatabaseError::Query(format!(
            "comando {number} falhou: {error} — {}",
            preview(statement)
        ))
    };

    if family == EngineFamily::Postgres && is_copy_from_stdin(statement) {
        progress.stage(&preview(statement));
        let rows = session.copy_from(statement, input).await.map_err(fail)?;
        stats.rows += rows;
        progress.rows(rows);
    } else {
        if is_ddl(statement) {
            progress.stage(&preview(statement));
        }
        let affected = session.execute(statement).await.map_err(fail)?;
        if starts_with_keyword(statement, "INSERT") {
            stats.rows += affected;
            progress.rows(affected);
        }
    }
    Ok(())
}

fn starts_with_keyword(statement: &str, keyword: &str) -> bool {
    statement
        .trim_start()
        .get(..keyword.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(keyword))
}

fn is_ddl(statement: &str) -> bool {
    ["CREATE", "ALTER"]
        .iter()
        .any(|keyword| starts_with_keyword(statement, keyword))
}

/// Começo de um comando numa linha só, para mensagem e andamento.
#[must_use]
pub fn preview(statement: &str) -> String {
    let single: String = statement.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut head: String = single.chars().take(100).collect();
    if head.len() < single.len() {
        head.push('…');
    }
    head
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::databases::NoProgress;

    /// Sessão que só anota o que recebeu.
    #[derive(Default)]
    struct Recorder {
        executed: Vec<String>,
        copied: Vec<String>,
        fail_on: Option<&'static str>,
    }

    #[async_trait]
    impl RestoreSession for Recorder {
        async fn execute(&mut self, statement: &str) -> Result<u64, DatabaseError> {
            if self
                .fail_on
                .is_some_and(|needle| statement.contains(needle))
            {
                return Err(DatabaseError::Query("syntax error".into()));
            }
            self.executed.push(statement.to_string());
            Ok(if statement.starts_with("INSERT") {
                2
            } else {
                0
            })
        }

        async fn copy_from(
            &mut self,
            _statement: &str,
            input: &mut LineReader,
        ) -> Result<u64, DatabaseError> {
            let mut rows = 0;
            while let Some(line) = input.next_line().await? {
                if line == "\\." {
                    break;
                }
                self.copied.push(line);
                rows += 1;
            }
            Ok(rows)
        }
    }

    fn reader(sql: &str, checksum: Option<String>) -> LineReader {
        LineReader::plain(
            Box::pin(std::io::Cursor::new(sql.as_bytes().to_vec())),
            checksum,
        )
    }

    #[tokio::test]
    async fn aplica_comandos_e_entrega_o_copy_ao_servidor() {
        let sql = "CREATE TABLE t (a int);\nCOPY \"public\".\"t\" (\"a\") FROM stdin;\n1\n2\n\\.\nSELECT 1;\n";
        let mut session = Recorder::default();
        let stats = apply_script(
            &mut session,
            EngineFamily::Postgres,
            &mut reader(sql, None),
            &NoProgress,
        )
        .await
        .unwrap();

        assert_eq!(session.executed, vec!["CREATE TABLE t (a int)", "SELECT 1"]);
        assert_eq!(session.copied, vec!["1", "2"]);
        assert_eq!(
            stats,
            RestoreStats {
                statements: 3,
                rows: 2
            }
        );
    }

    #[tokio::test]
    async fn soma_que_nao_confere_e_recusada() {
        let mut session = Recorder::default();
        let result = apply_script(
            &mut session,
            EngineFamily::Mysql,
            &mut reader("SELECT 1;\n", Some("00".repeat(32))),
            &NoProgress,
        )
        .await;
        assert!(matches!(result, Err(DatabaseError::Checksum)));
    }

    #[tokio::test]
    async fn soma_certa_passa() {
        let sql = "SELECT 1;\n";
        let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
        let mut session = Recorder::default();
        apply_script(
            &mut session,
            EngineFamily::Mysql,
            &mut reader(sql, Some(checksum)),
            &NoProgress,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn a_falha_diz_qual_comando_e_o_comeco_dele() {
        let mut session = Recorder {
            fail_on: Some("quebrado"),
            ..Recorder::default()
        };
        let error = apply_script(
            &mut session,
            EngineFamily::Mysql,
            &mut reader("SELECT 1;\nINSERT INTO quebrado VALUES (1);\n", None),
            &NoProgress,
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(error.contains("comando 2"), "{error}");
        assert!(error.contains("INSERT INTO quebrado"), "{error}");
    }

    #[test]
    fn a_previa_cabe_numa_linha() {
        assert_eq!(
            preview("CREATE  TABLE\n  t (a int)"),
            "CREATE TABLE t (a int)"
        );
        assert!(preview(&"x".repeat(300)).ends_with('…'));
    }
}
