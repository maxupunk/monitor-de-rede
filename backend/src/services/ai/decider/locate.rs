//! Onde está o Ollaya do compose.
//!
//! A URL padrão (`http://ollaya:11435`) é o nome do serviço na rede interna do
//! compose — e só vale ali. Com o override de rede do host
//! (`docker-compose.host.yml`, o que o syslog pede para ver o IP real dos
//! roteadores) a API sai dessa rede e o nome deixa de resolver, embora o
//! Ollaya esteja de pé. Instalações montadas à mão (um painel de containers,
//! outra rede) caem no mesmo buraco.
//!
//! Por isso, enquanto a tela estiver com a URL padrão, o servidor procura o
//! Ollaya nos endereços em que o compose o deixa: `OLLAYA_URL` (o override de
//! host a define), o nome do serviço, o loopback (porta publicada em
//! `127.0.0.1`) e o host do Docker. Uma URL digitada na tela é escolha do
//! operador e nunca é trocada.

use std::{
    borrow::Cow,
    sync::{OnceLock, RwLock},
    time::{Duration, Instant},
};

use futures::future::join_all;

use super::{
    config::{AiDeciderSettings, DEFAULT_DECIDER_BASE_URL},
    providers::DeciderProvider,
};

/// Endereço do Ollaya definido pela instalação — vem antes dos palpites.
pub const URL_ENV: &str = "OLLAYA_URL";

/// Onde o compose deixa o Ollaya, depois do nome do serviço.
const FALLBACKS: [&str; 2] = [
    "http://127.0.0.1:11435",
    "http://host.docker.internal:11435",
];

/// Quanto esperar cada endereço. Nome que não resolve e porta fechada falham
/// na hora; o teto só pesa num endereço que engole pacotes.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1_500);

/// Quanto tempo um endereço achado vale sem nova procura.
const REMEMBER: Duration = Duration::from_secs(5 * 60);

fn found() -> &'static RwLock<Option<(String, Instant)>> {
    static FOUND: OnceLock<RwLock<Option<(String, Instant)>>> = OnceLock::new();
    FOUND.get_or_init(|| RwLock::new(None))
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Os endereços a tentar, na ordem, sem repetição.
fn candidates(env: Option<&str>) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    let env = env
        .map(|value| value.trim().trim_end_matches('/'))
        .filter(|value| !value.is_empty());
    for url in env
        .into_iter()
        .chain([DEFAULT_DECIDER_BASE_URL])
        .chain(FALLBACKS)
    {
        if !list.iter().any(|known| known == url) {
            list.push(url.to_owned());
        }
    }
    list
}

/// Procurar só quando a URL é a padrão do Ollaya — a da tela é soberana.
fn searches(settings: &AiDeciderSettings) -> bool {
    settings.provider == DeciderProvider::Ollaya
        && settings.base_url.trim_end_matches('/') == DEFAULT_DECIDER_BASE_URL
}

/// Qualquer resposta HTTP conta, até um 401: quem respondeu é o Ollaya, e a
/// chave é outro problema, com a sua própria mensagem.
async fn answers(url: &str) -> bool {
    http()
        .get(format!("{url}/api/tags"))
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .is_ok()
}

fn remembered() -> Option<String> {
    let guard = found().read().ok()?;
    guard
        .as_ref()
        .filter(|(_, at)| at.elapsed() < REMEMBER)
        .map(|(url, _)| url.clone())
}

/// O primeiro endereço que responde, em paralelo. Nenhum respondendo, fica o
/// primeiro da lista — e a mensagem de erro mostra os que foram tentados.
async fn locate() -> Result<String, Vec<String>> {
    if let Some(url) = remembered() {
        return Ok(url);
    }
    let list = candidates(std::env::var(URL_ENV).ok().as_deref());
    let replies = join_all(list.iter().map(|url| answers(url))).await;
    let Some(url) = list
        .iter()
        .zip(replies)
        .find_map(|(url, ok)| ok.then(|| url.clone()))
    else {
        return Err(list);
    };
    if let Ok(mut guard) = found().write() {
        *guard = Some((url.clone(), Instant::now()));
    }
    if url != DEFAULT_DECIDER_BASE_URL {
        tracing::info!(%url, "decisor: Ollaya encontrado fora do nome do serviço");
    }
    Ok(url)
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
    match locate().await {
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

    #[test]
    fn a_variavel_vem_antes_e_nada_se_repete() {
        assert_eq!(
            candidates(Some(" http://127.0.0.1:11435/ ")),
            vec![
                "http://127.0.0.1:11435",
                "http://ollaya:11435",
                "http://host.docker.internal:11435",
            ]
        );
        assert_eq!(
            candidates(Some("  ")).first().unwrap(),
            DEFAULT_DECIDER_BASE_URL
        );
    }

    #[test]
    fn so_procura_com_a_url_padrao_do_ollaya() {
        let padrao = AiDeciderSettings::default();
        assert!(searches(&padrao));
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
