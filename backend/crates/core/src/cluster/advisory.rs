//! A trava consultiva do PostgreSQL que elege o líder (ADR 024).
//!
//! `pg_try_advisory_lock` é **de sessão**: vale enquanto a conexão que a pegou
//! viver. Por isso ela mora numa conexão **dedicada**, tirada do pool
//! (`detach`) e nunca devolvida — uma conexão do pool voltaria para outro uso
//! ainda segurando a trava, ou seria reciclada e a soltaria em silêncio.
//!
//! A sessão é configurada como um *lease*:
//!
//! * `idle_session_timeout` — se o processo líder parar de conferir a trava
//!   (travado, sem rede até o banco), o PostgreSQL encerra a sessão e solta a
//!   trava sozinho; outro processo assume na tentativa seguinte;
//! * `tcp_keepalives_*` — a máquina do líder que some sem fechar o TCP é
//!   descoberta em segundos, e não nas duas horas do padrão do sistema.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use sqlx::{Connection, PgConnection};
use tokio::sync::Mutex;

use super::leadership::LeaderLock;
use crate::shared::errors::{AppError, AppResult};

/// Configuração da sessão dedicada. O tempo ocioso precisa ficar bem acima da
/// conferência do líder ([`super::leadership::ElectorTiming`]).
const SESSION_SETUP: &str = "SET application_name = 'netmonitor-lider'; \
     SET idle_session_timeout = '30s'; \
     SET tcp_keepalives_idle = 10; \
     SET tcp_keepalives_interval = 5; \
     SET tcp_keepalives_count = 3;";

/// A trava de uma função, numa conexão só dela.
pub struct PgAdvisoryLock {
    db: DatabaseConnection,
    key: i64,
    session: Mutex<Option<PgConnection>>,
}

impl PgAdvisoryLock {
    #[must_use]
    pub fn new(db: DatabaseConnection, key: i64) -> Self {
        Self {
            db,
            key,
            session: Mutex::new(None),
        }
    }

    async fn open(&self) -> Result<PgConnection, sqlx::Error> {
        let mut conn = self
            .db
            .get_postgres_connection_pool()
            .acquire()
            .await?
            .detach();
        sqlx::raw_sql(SESSION_SETUP).execute(&mut conn).await?;
        Ok(conn)
    }
}

fn db_error(error: sqlx::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(error))
}

#[async_trait]
impl LeaderLock for PgAdvisoryLock {
    async fn try_acquire(&self) -> AppResult<bool> {
        let mut session = self.session.lock().await;
        if session.is_none() {
            *session = Some(self.open().await.map_err(db_error)?);
        }
        let Some(conn) = session.as_mut() else {
            return Ok(false);
        };
        match sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
            .bind(self.key)
            .fetch_one(&mut *conn)
            .await
        {
            Ok(acquired) => Ok(acquired),
            Err(error) => {
                // Sessão quebrada: a próxima tentativa abre outra.
                *session = None;
                Err(db_error(error))
            }
        }
    }

    async fn still_held(&self) -> AppResult<bool> {
        let mut session = self.session.lock().await;
        let Some(conn) = session.as_mut() else {
            return Ok(false);
        };
        // Consultar `pg_locks` pela própria sessão prova as duas coisas de uma
        // vez: a conexão responde e a trava continua dela.
        let held = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_locks \
              WHERE locktype = 'advisory' AND pid = pg_backend_pid() AND granted \
                AND objsubid = 1 \
                AND classid::bigint = ($1::bigint >> 32) \
                AND objid::bigint = ($1::bigint & 4294967295))",
        )
        .bind(self.key)
        .fetch_one(&mut *conn)
        .await;
        match held {
            Ok(held) => Ok(held),
            Err(error) => {
                *session = None;
                Err(db_error(error))
            }
        }
    }

    async fn release(&self) {
        // Fechar a sessão solta a trava; se ela já caiu, não há o que soltar.
        if let Some(conn) = self.session.lock().await.take() {
            let _ = conn.close().await;
        }
    }
}
