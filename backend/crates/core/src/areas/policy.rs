//! O que cada área sabe sobre si mesma.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Area;
use crate::shared::errors::AppResult;

/// Sinal persistido de que a instalação usa a área.
///
/// Nunca é estado volátil (online/offline): a queda de um agente não pode
/// fazer uma área "sumir" da evidência no meio de um incidente. E nunca conta
/// o que o próprio sistema cria sozinho — o agente local, o dispositivo do
/// sistema, os DNS padrão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Evidence {
    pub present: bool,
    /// Por que há evidência, em uma frase ("2 redes IP cadastradas").
    pub reason: Option<String>,
}

impl Evidence {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn because(reason: impl Into<String>) -> Self {
        Self {
            present: true,
            reason: Some(reason.into()),
        }
    }
}

/// Efeito concreto de desligar a área, para a confirmação na tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Consequence {
    pub message: String,
}

impl Consequence {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// O que impede desligar a área agora, e como resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Blocker {
    pub message: String,
}

/// O conhecimento de uma área sobre si mesma. Uma implementação por área
/// opcional, registrada pelo ponto de montagem.
#[async_trait]
pub trait AreaPolicy: Send + Sync {
    fn area(&self) -> Area;

    /// A instalação usa esta área? Serve às sugestões ("Encontramos um host
    /// Docker. Mostrar a área Servidores?"); nunca liga nem desliga sozinha.
    async fn evidence(&self, db: &DatabaseConnection) -> AppResult<Evidence>;

    /// O que muda ao desligar. Lembre que monitores **nunca** param pela área.
    async fn consequences(&self, db: &DatabaseConnection) -> AppResult<Vec<Consequence>>;

    /// Algo impede desligar agora? `None` libera.
    async fn blocker(&self, _db: &DatabaseConnection) -> AppResult<Option<Blocker>> {
        Ok(None)
    }

    /// As rotas da API desta área (ADR 019): quem não tem acesso à área recebe
    /// 403 nelas. `*` vale um segmento (`/api/devices/*/snmp`); o último
    /// segmento casa por prefixo (`/api/plugin` cobre `/api/plugin-batches`).
    fn api_prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    /// Os prefixos dos eventos do SSE desta área (`docker:`): não chegam a
    /// quem não tem acesso a ela.
    fn event_prefixes(&self) -> &'static [&'static str] {
        &[]
    }
}

/// O caminho casa com o padrão de rota de uma área? Ver
/// [`AreaPolicy::api_prefixes`].
#[must_use]
pub fn path_matches(pattern: &str, path: &str) -> bool {
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let padrao: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let caminho: Vec<&str> = path.split('/').collect();
    let Some((ultimo, antes)) = padrao.split_last() else {
        return false;
    };
    caminho.len() >= padrao.len()
        && antes
            .iter()
            .zip(&caminho)
            .all(|(esperado, real)| *esperado == "*" || esperado == real)
        && caminho[antes.len()].starts_with(ultimo)
}

/// Monitores nunca param pela área — a confirmação diz quantos seguem rodando.
#[must_use]
pub fn monitors_keep_running(count: u64, singular: &str, plural_form: &str) -> Option<Consequence> {
    (count > 0).then(|| {
        Consequence::new(format!(
            "{} continua{} rodando e alertando: monitor nunca para pela área.",
            crate::shared::text::count_noun(count, singular, plural_form),
            if count == 1 { "" } else { "m" }
        ))
    })
}

/// As políticas das áreas opcionais, registradas pelo ponto de montagem no
/// `shared_store` do processo (o `after_context`).
#[derive(Clone, Default)]
pub struct AreaPolicies(std::sync::Arc<Vec<Box<dyn AreaPolicy>>>);

impl AreaPolicies {
    #[must_use]
    pub fn new(policies: Vec<Box<dyn AreaPolicy>>) -> Self {
        Self(std::sync::Arc::new(policies))
    }

    /// As do processo; vazio quando o ponto de montagem não registrou nada.
    #[must_use]
    pub fn from_context(ctx: &loco_rs::app::AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }
}

impl AreaPolicies {
    /// A área de uma rota da API; o Núcleo quando nenhuma área a declara.
    #[must_use]
    pub fn area_of_path(&self, path: &str) -> Area {
        self.0
            .iter()
            .find(|policy| {
                policy
                    .api_prefixes()
                    .iter()
                    .any(|pattern| path_matches(pattern, path))
            })
            .map_or(Area::Core, |policy| policy.area())
    }

    /// A área de um evento do SSE; o Núcleo quando nenhuma área o declara.
    #[must_use]
    pub fn area_of_event(&self, event_type: &str) -> Area {
        self.0
            .iter()
            .find(|policy| {
                policy
                    .event_prefixes()
                    .iter()
                    .any(|prefix| event_type.starts_with(prefix))
            })
            .map_or(Area::Core, |policy| policy.area())
    }
}

impl std::ops::Deref for AreaPolicies {
    type Target = [Box<dyn AreaPolicy>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod path_tests {
    use super::path_matches;

    #[test]
    fn padrao_casa_por_segmento_com_curinga_e_prefixo_no_fim() {
        assert!(path_matches("/api/docker", "/api/docker"));
        assert!(path_matches("/api/docker", "/api/docker/hosts/agent-1"));
        assert!(path_matches("/api/plugin", "/api/plugin-batches/3"));
        assert!(path_matches(
            "/api/devices/*/snmp",
            "/api/devices/7/snmp/scan"
        ));
        assert!(path_matches("/api/vpn", "/api/vpn/peers?x=1"));
        assert!(!path_matches(
            "/api/devices/*/snmp",
            "/api/devices/7/interfaces"
        ));
        assert!(!path_matches("/api/devices/*/snmp", "/api/devices"));
        assert!(!path_matches("/api/docker", "/api/dashboard"));
    }
}
