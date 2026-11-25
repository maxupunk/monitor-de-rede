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
use netmonitor_core::events::EventBus;

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

struct JobState {
    snapshot: DatabaseJobSnapshot,
    last_publish: Option<Instant>,
}

struct Inner {
    state: Mutex<JobState>,
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
                state: Mutex::new(JobState {
                    snapshot: DatabaseJobSnapshot {
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
                    },
                    last_publish: None,
                }),
                bus,
            }),
        };
        job.publish_snapshot(job.snapshot());
        job
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, JobState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[must_use]
    pub fn snapshot(&self) -> DatabaseJobSnapshot {
        self.lock().snapshot.clone()
    }

    fn update(&self, change: impl FnOnce(&mut DatabaseJobSnapshot), force: bool) {
        let to_publish = {
            let mut guard = self.lock();
            change(&mut guard.snapshot);
            if self.inner.bus.is_none() {
                None
            } else if force
                || guard
                    .last_publish
                    .is_none_or(|at| at.elapsed() >= PUBLISH_EVERY)
            {
                guard.last_publish = Some(Instant::now());
                Some(guard.snapshot.clone())
            } else {
                None
            }
        };
        if let Some(snapshot) = to_publish {
            self.publish_snapshot(snapshot);
        }
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

    /// Zera contadores de linhas/bytes do banco atual antes de uma nova
    /// tentativa (ex.: queda do canal do agente durante dump).
    pub fn reset_database_progress(&self) {
        self.update(
            |state| {
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

    /// Etapa de alto nível da execução ("Enviando ao armazenamento"): publica
    /// na hora, porque o passo seguinte pode demorar sem nenhum outro aviso.
    pub fn milestone(&self, label: &str) {
        self.update(|state| label.clone_into(&mut state.stage), true);
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

    fn publish_snapshot(&self, snapshot: DatabaseJobSnapshot) {
        let Some(bus) = &self.inner.bus else {
            return;
        };
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
    /// Etapa de baixo nível (tabela, comando do script): respeita
    /// [`PUBLISH_EVERY`] — a restauração chama isto a cada DDL/`COPY`, e
    /// publicar cada troca de rótulo inundaria o SSE. As etapas de alto nível
    /// vão por [`Job::milestone`].
    fn stage(&self, label: &str) {
        self.update(
            |state| {
                if state.stage != label {
                    label.clone_into(&mut state.stage);
                }
            },
            false,
        );
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
        job.reset_database_progress();
        assert_eq!(job.snapshot().rows, 0);
        assert_eq!(job.snapshot().bytes, 0);
        job.rows(7);
        assert_eq!(job.snapshot().rows, 7);
        job.database_done();
        job.begin_database("estoque");
        let snapshot = job.snapshot();
        assert_eq!(snapshot.rows, 0);
        assert_eq!(snapshot.databases_done, 1);
        assert_eq!(snapshot.database.as_deref(), Some("estoque"));
    }

    /// Esvazia o receptor e devolve quantas publicações do job chegaram.
    fn drain(
        rx: &mut tokio::sync::broadcast::Receiver<netmonitor_core::events::DomainEvent>,
    ) -> usize {
        let mut count = 0;
        while let Ok(event) = rx.try_recv() {
            if event.event_type == EVENT {
                count += 1;
            }
        }
        count
    }

    #[test]
    fn trocar_de_etapa_respeita_o_intervalo_e_o_marco_publica_na_hora() {
        let bus = EventBus::create();
        let mut rx = bus.subscribe();
        let job = Job::start(Some(bus.clone()), DatabaseJobKind::Restore, 9, 1);
        job.begin_database("vendas");
        drain(&mut rx);
        for n in 0..500 {
            job.stage(&format!("CREATE TABLE t{n}"));
        }
        assert_eq!(
            drain(&mut rx),
            0,
            "etapa de baixo nível não fura o intervalo"
        );
        assert_eq!(job.snapshot().stage, "CREATE TABLE t499");
        job.milestone("Enviando ao armazenamento");
        assert_eq!(drain(&mut rx), 1);
        job.finish(true, "ok");
        assert_eq!(drain(&mut rx), 1);
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
