//! Sessões de agentes conectadas a este processo.
//!
//! Vive no `shared_store` do processo da API — é ele que atende o WebSocket.
//! Um processo `task scheduler_loop` separado não tem hub: as tarefas de
//! monitor continuam indo pelo banco (`probe_tasks`) e o pump deste processo
//! as entrega (ver [`super::pump`]).

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
};

use loco_rs::app::AppContext;

use crate::services::shared::errors::{AppError, AppResult};

use super::session::AgentSession;

#[derive(Clone, Default)]
pub struct AgentHub {
    sessions: Arc<RwLock<HashMap<i64, Arc<AgentSession>>>>,
    live_interval_ms: Arc<AtomicU64>,
}

impl AgentHub {
    pub fn install(ctx: &AppContext) {
        if !ctx.shared_store.contains::<Self>() {
            ctx.shared_store.insert(Self::default());
        }
    }

    /// # Errors
    ///
    /// Hub ausente (processo que não atende agentes).
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store
            .get::<Self>()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Hub de agentes não inicializado")))
    }

    /// Registra a sessão. Uma conexão nova do mesmo agente substitui a antiga,
    /// que é fechada — o agente reconectou antes de a queda ser percebida.
    pub fn register(&self, session: Arc<AgentSession>) {
        let previous = self.write().insert(session.probe_id, session);
        if let Some(previous) = previous {
            previous.close();
        }
    }

    /// Remove a sessão **se ainda for a mesma**: a queda de uma conexão já
    /// substituída não pode derrubar a nova.
    pub fn unregister(&self, session: &Arc<AgentSession>) -> bool {
        let mut sessions = self.write();
        let is_current = sessions
            .get(&session.probe_id)
            .is_some_and(|current| Arc::ptr_eq(current, session));
        if is_current {
            sessions.remove(&session.probe_id);
        }
        drop(sessions);
        session.close();
        is_current
    }

    #[must_use]
    pub fn get(&self, probe_id: i64) -> Option<Arc<AgentSession>> {
        self.read().get(&probe_id).cloned()
    }

    #[must_use]
    pub fn sessions(&self) -> Vec<Arc<AgentSession>> {
        self.read().values().cloned().collect()
    }

    #[must_use]
    pub fn live_interval_ms(&self) -> u64 {
        self.live_interval_ms.load(Ordering::SeqCst)
    }

    /// Guarda o intervalo vigente; devolve se mudou.
    pub fn set_live_interval_ms(&self, value: u64) -> bool {
        self.live_interval_ms.swap(value, Ordering::SeqCst) != value
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<i64, Arc<AgentSession>>> {
        self.sessions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<i64, Arc<AgentSession>>> {
        self.sessions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::services::agents::{policy::Permission, session::tests::hello};

    fn session(id: i64) -> Arc<AgentSession> {
        let (tx, _rx) = mpsc::channel(1);
        Arc::new(AgentSession::new(
            id,
            "srv",
            hello(vec![Permission::Read]),
            tx,
        ))
    }

    #[test]
    fn reconexao_substitui_e_a_queda_antiga_nao_derruba_a_nova() {
        let hub = AgentHub::default();
        let old = session(1);
        let new = session(1);
        hub.register(old.clone());
        hub.register(new.clone());
        assert!(old.is_closed(), "a conexão substituída é fechada");
        assert!(!hub.unregister(&old), "a queda da antiga não remove a nova");
        assert!(Arc::ptr_eq(&hub.get(1).expect("nova"), &new));
        assert!(hub.unregister(&new));
        assert!(hub.get(1).is_none());
    }

    #[test]
    fn intervalo_ao_vivo_informa_mudanca() {
        let hub = AgentHub::default();
        assert!(hub.set_live_interval_ms(3_000));
        assert!(!hub.set_live_interval_ms(3_000));
        assert_eq!(hub.live_interval_ms(), 3_000);
    }
}
