//! Quem está com o NetMonitor aberto agora.
//!
//! Estar "online" é ter o stream SSE global aberto (`GET /api/events/stream`):
//! toda aba logada mantém um. Cada conexão segura um [`PresenceGuard`]; quando
//! o navegador fecha a aba, o stream cai, o guarda sai de escopo e a contagem
//! desce — sem batimento nem consulta periódica (AGENTS §9).
//!
//! Só o processo da API povoa o registro; nos outros ele existe vazio.

use loco_rs::app::AppContext;

use super::counter::{ConnectionCounter, CounterGuard};

#[derive(Clone, Default)]
pub struct Presence {
    connections: ConnectionCounter<i64>,
}

/// Uma conexão aberta. Ao cair, desconta a do usuário.
pub type PresenceGuard = CounterGuard<i64>;

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
        self.connections.connect(user_id)
    }

    #[must_use]
    pub fn is_online(&self, user_id: i64) -> bool {
        self.connections.is_active(&user_id)
    }
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
