//! Andamento ao vivo de um backup ou de uma restauração de banco.
//!
//! Um dump grande leva minutos: a requisição devolve na hora (`202`) e o
//! andamento chega pelo barramento SSE como snapshot — `database_jobs:updated`,
//! uma chave por conexão e tipo. Quem abre a tela depois recebe o último estado
//! logo ao conectar (AGENTS §9), sem consultar nada por HTTP.
//!
//! A publicação é limitada a uma a cada [`PUBLISH_EVERY`]: o `COPY` de uma
//! tabela grande chama [`Progress::rows`] milhares de vezes por segundo.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use ts_rs::TS;

use super::Progress;
use crate::services::events::EventBus;

pub const EVENT: &str = "database_jobs:updated";

const PUBLISH_EVERY: Duration = Duration::from_millis(750);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DatabaseJobKind {
    Backup,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DatabaseJobStatus {
    Running,
    Success,
    Failed,
}

/// O que a tela mostra de uma execução em curso ou recém-terminada.
///
/// Sem host, usuário ou senha: o barramento SSE chega a toda sessão aberta.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseJobSnapshot {
    pub id: String,
    pub kind: DatabaseJobKind,
    #[ts(type = "number")]
    pub connection_id: i64,
    /// Banco em curso.
    pub database: Option<String>,
    pub stage: String,
    #[ts(type = "number")]
    pub rows: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    pub databases_done: u32,
    pub databases_total: u32,
    pub status: DatabaseJobStatus,
    pub message: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

/// Uma execução em andamento. Clonar compartilha o mesmo estado.
#[derive(Clone)]
pub struct Job {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<DatabaseJobSnapshot>,
    last_publish: Mutex<Option<Instant>>,
    bus: Option<EventBus>,
}

impl Job {
    /// Começa uma execução e já a publica.
    #[must_use]
    pub fn start(
        bus: Option<EventBus>,
        kind: DatabaseJobKind,
        connection_id: i64,
        databases_total: u32,
    ) -> Self {
        let job = Self {
            inner: Arc::new(Inner {
                state: Mutex::new(DatabaseJobSnapshot {
                    id: uuid::Uuid::new_v4().to_string(),
                    kind,
                    connection_id,
                    database: None,
                    stage: "Conectando".to_string(),
                    rows: 0,
                    bytes: 0,
                    databases_done: 0,
                    databases_total,
                    status: DatabaseJobStatus::Running,
                    message: None,
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: None,
                }),
                last_publish: Mutex::new(None),
                bus,
            }),
        };
        job.publish(true);
        job
    }

    #[must_use]
    pub fn snapshot(&self) -> DatabaseJobSnapshot {
        self.inner
            .state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    fn update(&self, change: impl FnOnce(&mut DatabaseJobSnapshot), force: bool) {
        if let Ok(mut state) = self.inner.state.lock() {
            change(&mut state);
        }
        self.publish(force);
    }

    /// Passou para o banco `database`; zera os contadores do anterior.
    pub fn begin_database(&self, database: &str) {
        self.update(
            |state| {
                state.database = Some(database.to_string());
                state.stage = "Conectando".to_string();
                state.rows = 0;
                state.bytes = 0;
            },
            true,
        );
    }

    pub fn database_done(&self) {
        self.update(|state| state.databases_done += 1, true);
    }

    pub fn total(&self, databases_total: u32) {
        self.update(|state| state.databases_total = databases_total, true);
    }

    /// Fim, com a mensagem que a tela mostra.
    pub fn finish(&self, ok: bool, message: impl Into<String>) {
        let message = message.into();
        self.update(
            |state| {
                state.status = if ok {
                    DatabaseJobStatus::Success
                } else {
                    DatabaseJobStatus::Failed
                };
                state.stage = if ok { "Concluído" } else { "Falhou" }.to_string();
                state.message = Some(message);
                state.finished_at = Some(Utc::now().to_rfc3339());
            },
            true,
        );
    }

    fn publish(&self, force: bool) {
        let Some(bus) = &self.inner.bus else {
            return;
        };
        if let Ok(mut last) = self.inner.last_publish.lock() {
            if !force && last.is_some_and(|at| at.elapsed() < PUBLISH_EVERY) {
                return;
            }
            *last = Some(Instant::now());
        }
        let snapshot = self.snapshot();
        let key = format!(
            "{}:{}",
            match snapshot.kind {
                DatabaseJobKind::Backup => "backup",
                DatabaseJobKind::Restore => "restore",
            },
            snapshot.connection_id
        );
        bus.publish_snapshot(EVENT, key, serde_json::json!(snapshot));
    }
}

impl Progress for Job {
    fn stage(&self, label: &str) {
        self.update(|state| state.stage = label.to_string(), false);
    }

    fn rows(&self, count: u64) {
        self.update(|state| state.rows += count, false);
    }

    fn bytes(&self, total: u64) {
        self.update(|state| state.bytes = total, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acumula_linhas_e_zera_ao_trocar_de_banco() {
        let job = Job::start(None, DatabaseJobKind::Backup, 1, 2);
        job.begin_database("vendas");
        job.rows(10);
        job.rows(5);
        job.bytes(1024);
        assert_eq!(job.snapshot().rows, 15);
        job.database_done();
        job.begin_database("estoque");
        let snapshot = job.snapshot();
        assert_eq!(snapshot.rows, 0);
        assert_eq!(snapshot.databases_done, 1);
        assert_eq!(snapshot.database.as_deref(), Some("estoque"));
    }

    #[test]
    fn o_fim_registra_status_e_instante() {
        let job = Job::start(None, DatabaseJobKind::Restore, 3, 1);
        job.finish(false, "senha errada");
        let snapshot = job.snapshot();
        assert_eq!(snapshot.status, DatabaseJobStatus::Failed);
        assert_eq!(snapshot.message.as_deref(), Some("senha errada"));
        assert!(snapshot.finished_at.is_some());
    }
}
