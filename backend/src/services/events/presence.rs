//! Quem está com o NetMonitor aberto agora.
//!
//! Estar "online" é ter o stream SSE global aberto (`GET /api/events/stream`):
//! toda aba logada mantém um. Cada conexão segura um [`PresenceGuard`]; quando
//! o navegador fecha a aba, o stream cai, o guarda sai de escopo e a contagem
//! desce — sem batimento nem consulta periódica (AGENTS §9).
//!
//! Só o processo da API povoa o registro; nos outros ele existe vazio.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use loco_rs::app::AppContext;

type Connections = Arc<Mutex<HashMap<i64, usize>>>;

#[derive(Clone, Default)]
pub struct Presence {
    connections: Connections,
}

impl Presence {
    /// Registra a instância do processo no `shared_store`.
    pub fn install(ctx: &AppContext) {
        ctx.shared_store.insert(Self::default());
    }

    /// A instância do processo. Sem ela instalada, ninguém conta como online.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }

    /// Marca mais uma conexão do usuário, até o guarda sair de escopo.
    #[must_use]
    pub fn connect(&self, user_id: i64) -> PresenceGuard {
        *lock(&self.connections).entry(user_id).or_default() += 1;
        PresenceGuard {
            connections: Arc::clone(&self.connections),
            user_id,
        }
    }

    #[must_use]
    pub fn is_online(&self, user_id: i64) -> bool {
        lock(&self.connections)
            .get(&user_id)
            .is_some_and(|count| *count > 0)
    }
}

/// Uma conexão aberta. Ao cair, desconta a do usuário.
pub struct PresenceGuard {
    connections: Connections,
    user_id: i64,
}

impl Drop for PresenceGuard {
    fn drop(&mut self) {
        let mut connections = lock(&self.connections);
        if let Some(count) = connections.get_mut(&self.user_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(&self.user_id);
            }
        }
    }
}

fn lock(connections: &Connections) -> std::sync::MutexGuard<'_, HashMap<i64, usize>> {
    connections.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conta_cada_aba_e_sai_quando_a_ultima_fecha() {
        let presence = Presence::default();
        assert!(!presence.is_online(7));

        let primeira = presence.connect(7);
        let segunda = presence.connect(7);
        assert!(presence.is_online(7));
        assert!(!presence.is_online(8), "outro usuário não conta");

        drop(primeira);
        assert!(presence.is_online(7), "ainda há uma aba aberta");
        drop(segunda);
        assert!(!presence.is_online(7));
    }
}
