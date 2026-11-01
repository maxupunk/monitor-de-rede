//! Onde está o Ollaya do compose.
//!
//! A procura em si é a de todo servidor de modelos do compose
//! ([`crate::services::ai::local_server`]); aqui fica só o que é do Decisor:
//! ela vale quando o provedor é o Ollaya e a URL da tela aponta para esta
//! máquina. O endereço de outra máquina e os provedores na nuvem são
//! soberanos.

use std::borrow::Cow;

use super::{config::AiDeciderSettings, providers::DeciderProvider};
use crate::services::ai::local_server::LocalServer;

/// O Ollaya do compose: o serviço `ollaya`, e `OLLAYA_URL` definida pela
/// instalação (o override de host a define).
pub static OLLAYA: LocalServer = LocalServer::new("Ollaya", "ollaya", 11435, "OLLAYA_URL");

/// Procurar só quando a URL aponta para esta máquina — a de outra é soberana.
fn searches(settings: &AiDeciderSettings) -> bool {
    settings.provider == DeciderProvider::Ollaya && OLLAYA.means_this_machine(&settings.base_url)
}

/// O resultado da procura, para a mensagem de erro dizer onde se procurou.
pub enum Located<'a> {
    /// A URL configurada, ou o endereço onde o Ollaya respondeu.
    Ready(Cow<'a, AiDeciderSettings>),
    /// Ninguém respondeu: a URL configurada, e os endereços tentados.
    NotFound(&'a AiDeciderSettings, Vec<String>),
}

/// As configurações com a URL onde o Ollaya de fato está.
pub async fn resolve(settings: &AiDeciderSettings) -> Located<'_> {
    if !searches(settings) {
        return Located::Ready(Cow::Borrowed(settings));
    }
    match OLLAYA.locate(&settings.base_url).await {
        Ok(url) if url == settings.base_url => Located::Ready(Cow::Borrowed(settings)),
        Ok(url) => Located::Ready(Cow::Owned(AiDeciderSettings {
            base_url: url,
            ..settings.clone()
        })),
        Err(tried) => Located::NotFound(settings, tried),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::decider::config::DEFAULT_DECIDER_BASE_URL;

    #[test]
    fn a_url_padrao_e_a_do_servico_do_compose() {
        assert_eq!(OLLAYA.default_url(), DEFAULT_DECIDER_BASE_URL);
    }

    #[test]
    fn procura_quando_a_url_aponta_para_esta_maquina() {
        let padrao = AiDeciderSettings::default();
        assert!(searches(&padrao));
        // Na bridge, `127.0.0.1` é o container: quem digitou quis dizer o host.
        for local in [
            "http://127.0.0.1:11435",
            "localhost:11435",
            "http://[::1]:11435",
            "http://host.docker.internal:11435",
        ] {
            let digitada = AiDeciderSettings {
                base_url: local.into(),
                ..AiDeciderSettings::default()
            };
            assert!(searches(&digitada), "{local}");
        }
        let digitada = AiDeciderSettings {
            base_url: "http://10.0.0.5:11435".into(),
            ..AiDeciderSettings::default()
        };
        assert!(!searches(&digitada));
        let nuvem = AiDeciderSettings {
            provider: DeciderProvider::Openrouter,
            ..AiDeciderSettings::default()
        };
        assert!(!searches(&nuvem));
    }
}
