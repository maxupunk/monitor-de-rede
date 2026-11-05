//! O que as abas abertas estão olhando (ARQ-05).
//!
//! Dado caro de produzir — o estado ao vivo do Docker, que obriga todos os
//! agentes a mandar snapshot a cada poucos segundos — só deve ser produzido
//! enquanto alguém o exibe. A aba declara o interesse **na própria conexão
//! SSE** (`GET /api/events/stream?interest=docker`): a contagem sobe com o
//! stream e desce quando ele cai, e a fonte consulta [`Interests::is_watched`].
//! A aba só reconecta quando o conjunto de interesses muda (ao entrar ou sair
//! de uma tela que usa o dado) — sem batimento nem consulta periódica.

use loco_rs::app::AppContext;

use super::counter::{ConnectionCounter, CounterGuard};

/// O estado ao vivo do Docker (inventário e consumo dos hosts).
pub const DOCKER: &str = "docker";

/// Os interesses que uma conexão pode declarar. O resto é ignorado: o nome
/// vem da URL e não pode criar chave arbitrária no registro.
pub const KNOWN: [&str; 1] = [DOCKER];

#[derive(Clone, Default)]
pub struct Interests {
    counter: ConnectionCounter<&'static str>,
}

/// Os interesses de uma conexão. Ao cair, descontam todos.
pub struct InterestGuard {
    _guards: Vec<CounterGuard<&'static str>>,
}

impl Interests {
    /// Registra a instância do processo no `shared_store`.
    pub fn install(ctx: &AppContext) {
        ctx.shared_store.insert(Self::default());
    }

    /// A instância do processo. Sem ela instalada, ninguém está olhando nada.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }

    /// Registra os interesses de uma conexão a partir da lista da URL
    /// (separada por vírgulas). Nomes desconhecidos e repetidos são ignorados.
    #[must_use]
    pub fn connect(&self, declared: &str) -> InterestGuard {
        let mut names: Vec<&'static str> = declared
            .split(',')
            .map(str::trim)
            .filter_map(|name| KNOWN.iter().copied().find(|known| *known == name))
            .collect();
        names.sort_unstable();
        names.dedup();
        InterestGuard {
            _guards: names
                .into_iter()
                .map(|name| self.counter.connect(name))
                .collect(),
        }
    }

    /// Alguma aba aberta exibe este dado agora?
    #[must_use]
    pub fn is_watched(&self, name: &str) -> bool {
        KNOWN
            .iter()
            .find(|known| **known == name)
            .is_some_and(|known| self.counter.is_active(known))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interesse_dura_enquanto_a_conexao_esta_aberta() {
        let interests = Interests::default();
        assert!(!interests.is_watched(DOCKER));
        let aba = interests.connect("docker");
        let outra = interests.connect("docker, docker");
        assert!(interests.is_watched(DOCKER));
        drop(aba);
        assert!(interests.is_watched(DOCKER), "ainda há uma aba olhando");
        drop(outra);
        assert!(!interests.is_watched(DOCKER));
    }

    #[test]
    fn nome_desconhecido_e_ignorado() {
        let interests = Interests::default();
        let _aba = interests.connect("qualquer,coisa");
        assert!(!interests.is_watched("qualquer"));
        assert!(!interests.is_watched(DOCKER));
    }
}
