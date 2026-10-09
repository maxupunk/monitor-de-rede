//! Backup de bancos de dados de terceiros — MySQL, MariaDB e PostgreSQL.
//!
//! ## Nativo: sem `pg_dump` nem `mysqldump`
//!
//! O dump sai pelo protocolo de cada SGBD, com o `sqlx`:
//!
//! * **PostgreSQL** — a estrutura vem do catálogo (`pg_get_functiondef`,
//!   `pg_get_constraintdef`, `pg_get_indexdef`, `pg_get_viewdef`…) e os dados
//!   por `COPY … TO STDOUT`, o mesmo caminho do `pg_dump`. Ver [`postgres`].
//! * **MySQL/MariaDB** — a estrutura vem de `SHOW CREATE` e os dados por
//!   `INSERT` estendido, com cada valor já citado **pelo próprio servidor**
//!   (`QUOTE`/`HEX`), sem conversão de tipo feita aqui. Ver [`mysql`].
//!
//! Não depender dos binários tira da imagem uma dependência que precisa casar
//! com a versão do servidor (`pg_dump` 15 recusa servidor 16), e deixa o
//! backup testável sem nada instalado.
//!
//! O arquivo é SQL comum compactado (`.sql.gz`): restaura por aqui
//! ([`restore`]) e também à mão, com `psql` ou `mysql`.
//!
//! ## Como as peças se encaixam
//!
//! | Peça | Responsabilidade |
//! |---|---|
//! | [`Driver`] | o que muda de um SGBD para outro: conectar, listar, dump, restaurar |
//! | [`sql`] | dividir um script em comandos e citar nomes — puro, sem rede |
//! | [`dump`] | o arquivo: SQL → SHA-256 → gzip → disco |
//! | [`restore`] | o laço que lê o arquivo e executa comando a comando |
//! | [`service`] | o cadastro das conexões, com a senha cifrada |
//! | [`backups`] | uma execução: dump de cada banco, envio, histórico, retenção |
//! | [`jobs`] | o andamento ao vivo, pelo SSE |
//! | [`schedule`] | o ciclo que dispara os backups automáticos |

pub mod backups;
pub mod dump;
pub mod jobs;
pub mod mysql;
pub mod postgres;
pub mod reach;
pub mod restore;
pub mod schedule;
pub mod service;
pub mod sql;

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::shared::{
    errors::{AppError, AppResult},
    runtime,
};
use dump::{DumpStats, DumpWriter};
use restore::{LineReader, RestoreStats};

/// Teto para abrir a conexão. Um host que engole pacotes em vez de recusar
/// não pode segurar a tela nem o agendador.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Os SGBDs suportados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DatabaseEngine {
    Postgres,
    Mysql,
    Mariadb,
}

/// O protocolo — o que de fato decide o código. MariaDB fala o de MySQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineFamily {
    Postgres,
    Mysql,
}

impl DatabaseEngine {
    pub const ALL: [Self; 3] = [Self::Postgres, Self::Mysql, Self::Mariadb];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Mariadb => "mariadb",
        }
    }

    /// Lê o valor da coluna.
    ///
    /// # Errors
    ///
    /// Valor que não é de nenhum SGBD suportado.
    pub fn parse(value: &str) -> AppResult<Self> {
        Self::ALL
            .into_iter()
            .find(|engine| engine.as_str() == value)
            .ok_or_else(|| AppError::validation(format!("Banco de dados não suportado: {value}")))
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::Mariadb => "MariaDB",
        }
    }

    #[must_use]
    pub const fn family(self) -> EngineFamily {
        match self {
            Self::Postgres => EngineFamily::Postgres,
            Self::Mysql | Self::Mariadb => EngineFamily::Mysql,
        }
    }

    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self.family() {
            EngineFamily::Postgres => 5432,
            EngineFamily::Mysql => 3306,
        }
    }

    /// O driver que fala com este SGBD.
    #[must_use]
    pub fn driver(self) -> &'static dyn Driver {
        match self.family() {
            EngineFamily::Postgres => &postgres::PostgresDriver,
            EngineFamily::Mysql => &mysql::MysqlDriver,
        }
    }
}

/// Como a conexão usa TLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum SslMode {
    /// Sem TLS.
    Disable,
    /// TLS se o servidor oferecer.
    Prefer,
    /// TLS obrigatório; falha se o servidor não tiver.
    Require,
}

impl SslMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
        }
    }

    /// Lê o valor da coluna.
    ///
    /// # Errors
    ///
    /// Valor desconhecido.
    pub fn parse(value: &str) -> AppResult<Self> {
        [Self::Disable, Self::Prefer, Self::Require]
            .into_iter()
            .find(|mode| mode.as_str() == value)
            .ok_or_else(|| AppError::validation(format!("Modo de TLS inválido: {value}")))
    }
}

/// Para onde conectar, com a senha já em claro.
///
/// Não deriva `Debug` de propósito: um `{:?}` num log de erro imprimiria a
/// senha do banco do cliente.
#[derive(Clone)]
pub struct DatabaseTarget {
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub ssl_mode: SslMode,
    /// Banco ao qual se conectar. `None` usa o de manutenção do SGBD.
    pub database: Option<String>,
    /// Nome do agente quando a conexão passa pela ponte dele (ADR 013) — e
    /// então `host:port` é a ponta local da ponte, não o banco.
    pub via: Option<String>,
}

impl DatabaseTarget {
    /// O mesmo servidor, outro banco.
    #[must_use]
    pub fn with_database(&self, database: Option<&str>) -> Self {
        Self {
            database: database.map(ToString::to_string),
            ..self.clone()
        }
    }
}

/// Resultado de "Testar conexão".
#[derive(Debug, Clone)]
pub struct Probe {
    pub latency_ms: i64,
    pub version: String,
    pub databases: Vec<String>,
}

/// Onde a restauração escreve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum RestoreMode {
    /// Cria um banco novo; recusa se o nome já existir.
    NewDatabase,
    /// Apaga o conteúdo do banco existente e restaura nele.
    Replace,
}

/// Andamento de um dump ou de uma restauração.
///
/// O driver só avisa; quem decide o que fazer com o aviso (publicar no SSE,
/// ignorar num teste) é quem o chamou.
pub trait Progress: Send + Sync {
    /// Começou uma etapa ("Tabela clientes", "Índices"…).
    fn stage(&self, label: &str);
    /// Mais `count` linhas copiadas.
    fn rows(&self, count: u64);
    /// Total de bytes (descompactados) processados até agora.
    fn bytes(&self, total: u64);
}

/// Andamento que ninguém acompanha.
pub struct NoProgress;

impl Progress for NoProgress {
    fn stage(&self, _label: &str) {}
    fn rows(&self, _count: u64) {}
    fn bytes(&self, _total: u64) {}
}

/// O que muda de um SGBD para outro.
///
/// Pequeno de propósito: as quatro operações que o cadastro, o backup e a
/// restauração precisam. Um SGBD novo é um módulo novo com este trait — nada
/// mais muda.
#[async_trait]
pub trait Driver: Send + Sync {
    /// Conecta, lê a versão e lista os bancos que o usuário enxerga.
    async fn probe(&self, target: &DatabaseTarget) -> Result<Probe, DatabaseError>;

    /// Escreve o dump de `database` em `out`.
    async fn dump(
        &self,
        target: &DatabaseTarget,
        database: &str,
        out: &mut DumpWriter,
        progress: &dyn Progress,
    ) -> Result<DumpStats, DatabaseError>;

    /// Lê um dump e o aplica em `database`, criando ou esvaziando o banco
    /// conforme `mode`.
    async fn restore(
        &self,
        target: &DatabaseTarget,
        database: &str,
        mode: RestoreMode,
        input: &mut LineReader,
        progress: &dyn Progress,
    ) -> Result<RestoreStats, DatabaseError>;
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    /// O servidor respondeu e recusou (senha, banco inexistente).
    #[error("Não foi possível conectar: {0}")]
    Connection(String),
    /// A rede não chegou ao servidor (recusa TCP, tempo esgotado).
    #[error("Não foi possível conectar: {0}")]
    Unreachable(String),
    /// A conexão já aberta caiu no meio do trabalho.
    #[error("A conexão com o banco caiu: {0}")]
    Dropped(String),
    #[error("{0}")]
    Query(String),
    #[error("Nome de banco inválido: {0}")]
    InvalidName(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("Falha de leitura/escrita: {0}")]
    Io(String),
    #[error("O arquivo do backup está corrompido: a soma de verificação não confere")]
    Checksum,
}

impl DatabaseError {
    pub(crate) fn query(error: &sqlx::Error) -> Self {
        match error {
            sqlx::Error::Io(_) => Self::Dropped(describe_sqlx(error)),
            _ => Self::Query(describe_sqlx(error)),
        }
    }

    /// Falha de caminho, não de conteúdo: repetir pode dar certo. Senha
    /// errada, SQL recusado ou disco cheio falhariam igual de novo.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Unreachable(_) | Self::Dropped(_))
    }

    pub(crate) fn io(error: impl std::fmt::Display) -> Self {
        Self::Io(error.to_string())
    }
}

/// Mensagem do servidor, sem o embrulho do `sqlx`.
///
/// O `Display` do `sqlx` antepõe textos em inglês ("error communicating with
/// database:") à causa, que é o que interessa ao operador.
fn describe_sqlx(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(db) => db.message().to_string(),
        sqlx::Error::Io(io) => io.to_string(),
        sqlx::Error::Tls(tls) => format!("falha no TLS: {tls}"),
        other => other.to_string(),
    }
}

/// A falha é do servidor de banco do cliente, e a mensagem dele é o que diz ao
/// operador se a senha está errada ou o banco não existe: vai inteira (`400`).
impl From<DatabaseError> for AppError {
    fn from(error: DatabaseError) -> Self {
        match error {
            DatabaseError::InvalidName(_) => Self::validation(error.to_string()),
            DatabaseError::Conflict(_) => Self::conflict(error.to_string()),
            _ => Self::business_rule(error.to_string()),
        }
    }
}

/// Aplica o teto de [`CONNECT_TIMEOUT`] a uma conexão com `target`.
///
/// Falha de rede num endereço local, com este servidor num container, ganha a
/// explicação de [`runtime::loopback_hint`]: "conexão recusada em 127.0.0.1"
/// sozinho leva o operador a procurar o problema no banco, que está no ar.
/// Pela ponte de um agente o endereço local é a própria ponte: a dica não
/// cabe, e a mensagem diz por qual agente a conexão passou.
pub(crate) async fn connect_with_timeout<T>(
    target: &DatabaseTarget,
    future: impl std::future::Future<Output = Result<T, sqlx::Error>>,
) -> Result<T, DatabaseError> {
    let error = match tokio::time::timeout(CONNECT_TIMEOUT, future).await {
        Ok(Ok(connection)) => return Ok(connection),
        // O servidor respondeu (senha errada, banco inexistente): o endereço
        // está certo, e a dica do container só confundiria.
        Ok(Err(sqlx::Error::Database(db))) => {
            return Err(DatabaseError::Connection(db.message().to_string()))
        }
        Ok(Err(error)) => describe_sqlx(&error),
        Err(_) => format!("tempo esgotado ({} s)", CONNECT_TIMEOUT.as_secs()),
    };
    Err(DatabaseError::Unreachable(
        match (&target.via, runtime::loopback_hint(&target.host)) {
            (Some(agent), _) => format!("{error} (pela ponte do agente {agent})"),
            (None, Some(hint)) => format!("{error}. {hint}"),
            (None, None) => error,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_valor_da_coluna_volta_ao_mesmo_sgbd() {
        for engine in DatabaseEngine::ALL {
            assert_eq!(DatabaseEngine::parse(engine.as_str()).unwrap(), engine);
        }
        assert!(DatabaseEngine::parse("oracle").is_err());
    }

    #[test]
    fn mariadb_fala_o_protocolo_do_mysql() {
        assert_eq!(DatabaseEngine::Mariadb.family(), EngineFamily::Mysql);
        assert_eq!(DatabaseEngine::Mariadb.default_port(), 3306);
        assert_eq!(DatabaseEngine::Postgres.default_port(), 5432);
    }

    #[test]
    fn falha_do_servidor_chega_a_tela_com_a_mensagem() {
        let error: AppError = DatabaseError::Query("relation \"x\" does not exist".into()).into();
        assert_eq!(error.status(), axum::http::StatusCode::BAD_REQUEST);
        assert!(error.to_string().contains("relation"));
    }
}
