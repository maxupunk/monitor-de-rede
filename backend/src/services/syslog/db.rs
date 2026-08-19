//! Conexão com o **banco de logs**, separado do principal.
//!
//! Instalado em `Hooks::after_context`, e não num `Initializer`, porque o
//! `run_task` do Loco não executa initializers: a purga de retenção roda no
//! ciclo do `scheduler`, que pode ser invocado como `task`, e sem a conexão ali
//! ela falharia em silêncio. Mesma armadilha da ADR 007.
//!
//! **A separação é uma necessidade do SQLite, não uma regra universal.** No
//! SQLite, apagar ~1 M de linhas segura o *write lock* do arquivo inteiro e
//! congelaria a gravação de `monitor_results`. O PostgreSQL tem MVCC e não
//! sofre disso — por isso, quando o banco principal é Postgres e
//! `SYSLOG_DB_URL` não foi definida, os logs moram no **mesmo** banco. Um
//! `SYSLOG_DB_URL` explícito sempre vence.

use loco_rs::app::AppContext;
use migration::{logs::LogsMigrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};

use crate::services::shared::errors::{AppError, AppResult};

/// A conexão do banco de logs no `shared_store`.
///
/// Newtype porque o `shared_store` indexa por tipo: um `DatabaseConnection`
/// solto colidiria com qualquer outra conexão que alguém venha a guardar lá.
#[derive(Clone)]
pub struct LogsDb(pub DatabaseConnection);

impl LogsDb {
    /// Recupera a conexão do contexto.
    ///
    /// # Errors
    ///
    /// Erro nomeado quando a ingestão está desligada ou o banco não abriu — a
    /// mensagem chega ao controller em vez de virar `unwrap` no caminho quente.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "Banco de logs não inicializado (SYSLOG_ENABLED=false?)"
            ))
        })
    }

    #[must_use]
    pub fn connection(&self) -> &DatabaseConnection {
        &self.0
    }
}

/// A URL do banco de logs.
///
/// Ordem de decisão:
/// 1. `SYSLOG_DB_URL`, quando definida — vence sempre;
/// 2. banco principal em Postgres → o **mesmo** banco (ver nota do módulo);
/// 3. banco principal em SQLite → `logs.sqlite` ao lado dele. Em produção o
///    principal é `/data/netmonitor.sqlite`, então o de logs nasce em
///    `/data/logs.sqlite` sem `/data` estar cravado no código — o que também
///    faz o desenvolvimento local funcionar sem um diretório que não existe no
///    Windows.
#[must_use]
pub fn resolve_url(database_url: &str) -> String {
    if let Ok(url) = std::env::var("SYSLOG_DB_URL") {
        let url = url.trim();
        if !url.is_empty() {
            return url.to_owned();
        }
    }
    if !database_url.starts_with("sqlite:") {
        return database_url.to_owned();
    }
    substitui_arquivo_sqlite(database_url, "logs.sqlite")
}

/// Troca o nome do arquivo numa URL de SQLite, preservando esquema, diretório e
/// query (`?mode=rwc`).
fn substitui_arquivo_sqlite(url: &str, arquivo: &str) -> String {
    let (caminho, query) = url.split_once('?').map_or((url, ""), |(c, q)| (c, q));
    let diretorio = caminho.rsplit_once('/').map_or("sqlite://", |(dir, _)| dir);
    let base = format!("{diretorio}/{arquivo}");
    if query.is_empty() {
        base
    } else {
        format!("{base}?{query}")
    }
}

/// Abre a conexão, aplica os PRAGMAs e roda as migrations.
///
/// **A ordem dos PRAGMAs importa e não é intercambiável**: `auto_vacuum` é
/// propriedade *do arquivo* e só pode ser definida enquanto o banco está vazio.
/// Depois que a primeira tabela nasce, mudá-la exige um `VACUUM` completo.
/// Rodar as migrations antes seria erro silencioso — o `PRAGMA` é aceito, não
/// faz nada, e o disco só deixa de ser devolvido meses depois, em produção.
///
/// # Errors
///
/// Propaga falha de conexão ou de migração.
pub async fn connect(url: &str) -> AppResult<LogsDb> {
    let mut opcoes = ConnectOptions::new(url.to_owned());
    // Banco em memória vive **por conexão**: com pool maior que 1, cada
    // conexão abriria um banco vazio diferente e a metade das leituras não
    // acharia nada. É o arranjo dos testes.
    if em_memoria(url) {
        opcoes.max_connections(1).min_connections(1);
    }
    let db = Database::connect(opcoes)
        .await
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;

    if db.get_database_backend() == DatabaseBackend::Sqlite {
        aplica_pragmas(&db).await;
    }

    LogsMigrator::up(&db, None)
        .await
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;

    Ok(LogsDb(db))
}

fn em_memoria(url: &str) -> bool {
    url.contains(":memory:") || url.contains("mode=memory")
}

/// `auto_vacuum` antes de tudo, `journal_mode` em seguida.
///
/// Falha aqui não derruba o boot: um banco sem WAL é mais lento, não
/// inutilizável — mesma decisão de `enable_sqlite_wal` no `app.rs`.
async fn aplica_pragmas(db: &DatabaseConnection) {
    for pragma in [
        "PRAGMA auto_vacuum = INCREMENTAL;",
        "PRAGMA journal_mode = WAL;",
    ] {
        let statement = Statement::from_string(DatabaseBackend::Sqlite, pragma);
        if let Err(error) = db.query_one_raw(statement).await {
            tracing::warn!(%error, pragma, "PRAGMA do banco de logs recusado");
        }
    }
}

/// Instala a conexão no `shared_store`. Chamado do `after_context`.
///
/// Não devolve `Result` de propósito: um erro aqui abortaria **todo** comando,
/// inclusive `db migrate` e `doctor`. Quem precisa do banco descobre no lugar
/// certo, por [`LogsDb::from_context`], que devolve erro nomeado.
/// **`SYSLOG_ENABLED` não é consultado aqui, de propósito.** O flag governa o
/// *listener* — quem abre porta —, e desistir do banco por causa dele fazia
/// "logs do servidor ativados por padrão" morrer em silêncio em toda
/// instalação que não recebe syslog de roteador. Sem banco de logs não há log
/// interno, não há aba de logs e não há retenção.
pub async fn install(ctx: &AppContext, database_url: &str) {
    // **Teste nunca toca em arquivo.** O `resolve_url` poria o banco de logs ao
    // lado do de teste, e ali ele seria um arquivo só, compartilhado por toda a
    // suíte: o `Hooks::truncate` não o alcança (é outro banco), então uma linha
    // gravada por um teste sobreviveria para o seguinte. Em memória, cada
    // contexto nasce com o seu — isolamento por construção, não por lembrar de
    // limpar. `SYSLOG_DB_URL` explícita continua vencendo, para o teste que
    // quiser um arquivo de verdade.
    let url = if ctx.environment == loco_rs::environment::Environment::Test {
        std::env::var("SYSLOG_DB_URL")
            .ok()
            .filter(|valor| !valor.trim().is_empty())
            .unwrap_or_else(|| "sqlite::memory:".to_owned())
    } else {
        resolve_url(database_url)
    };
    match connect(&url).await {
        Ok(db) => {
            ctx.shared_store.insert(db);
            tracing::info!("banco de logs pronto");
        }
        Err(error) => {
            tracing::warn!(%error, "não foi possível abrir o banco de logs; ingestão indisponível");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn o_banco_de_logs_nasce_ao_lado_do_principal() {
        std::env::remove_var("SYSLOG_DB_URL");
        assert_eq!(
            resolve_url("sqlite:///data/netmonitor.sqlite?mode=rwc"),
            "sqlite:///data/logs.sqlite?mode=rwc"
        );
        assert_eq!(
            resolve_url("sqlite://netmonitor_development.sqlite?mode=rwc"),
            "sqlite://logs.sqlite?mode=rwc"
        );
    }

    #[test]
    #[serial]
    fn no_postgres_os_logs_ficam_no_mesmo_banco() {
        // O motivo da separação é o *write lock* do SQLite; o MVCC do Postgres
        // não sofre disso, e um segundo banco só criaria operação extra.
        std::env::remove_var("SYSLOG_DB_URL");
        let url = "postgres://user:pass@localhost/netmonitor";
        assert_eq!(resolve_url(url), url);
    }

    #[test]
    #[serial]
    fn a_variavel_explicita_vence_sempre() {
        std::env::set_var("SYSLOG_DB_URL", "sqlite:///outro/lugar.sqlite?mode=rwc");
        assert_eq!(
            resolve_url("postgres://user:pass@localhost/netmonitor"),
            "sqlite:///outro/lugar.sqlite?mode=rwc"
        );
        // Valor vazio é o mesmo que ausente.
        std::env::set_var("SYSLOG_DB_URL", "   ");
        assert_eq!(
            resolve_url("sqlite:///data/netmonitor.sqlite?mode=rwc"),
            "sqlite:///data/logs.sqlite?mode=rwc"
        );
        std::env::remove_var("SYSLOG_DB_URL");
    }

    #[tokio::test]
    #[serial]
    async fn conecta_migra_e_deixa_a_tabela_de_pe() {
        std::env::remove_var("SYSLOG_DB_URL");
        let db = connect("sqlite::memory:").await.expect("conexão");
        let existe = db
            .connection()
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name='device_logs'",
            ))
            .await
            .expect("consulta");
        assert!(existe.is_some(), "a migration do banco de logs não rodou");
    }
}
