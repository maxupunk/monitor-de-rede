//! Onde está o Ollaya do compose.
//!
//! A URL padrão (`http://ollaya:11435`) é o nome do serviço na rede interna do
//! compose — e só vale ali. Com o override de rede do host
//! (`docker-compose.host.yml`, o que o syslog pede para ver o IP real dos
//! roteadores) a API sai dessa rede e o nome deixa de resolver, embora o
//! Ollaya esteja de pé. Instalações montadas à mão (um painel de containers,
//! outra rede) caem no mesmo buraco.
//!
//! Por isso, enquanto a tela apontar para "esta máquina" — a URL padrão, o
//! loopback (`127.0.0.1`, `localhost`) ou o host do Docker —, o servidor
//! procura o Ollaya nos endereços em que o compose o deixa: a URL da tela
//! primeiro, depois `OLLAYA_URL` (o override de host a define), o nome do
//! serviço, o loopback (porta publicada em `127.0.0.1`) e o host do Docker.
//! O loopback entra na procura porque o seu sentido muda com a rede do
//! container: na bridge, `127.0.0.1` é o próprio container, e quem digitou
//! quis dizer "esta máquina". O endereço de outra máquina é escolha do
//! operador e nunca é trocado.

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
use crate::services::shared::runtime::{is_loopback_host, DOCKER_HOST_ALIAS};

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

/// O último achado: a URL da tela, onde o Ollaya respondeu e quando.
type Found = Option<(String, String, Instant)>;

fn found() -> &'static RwLock<Found> {
    static FOUND: OnceLock<RwLock<Found>> = OnceLock::new();
    FOUND.get_or_init(|| RwLock::new(None))
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Os endereços a tentar, na ordem, sem repetição: o da tela, o da
/// instalação e os palpites.
fn candidates(configured: &str, env: Option<&str>) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    let clean = |value: &str| value.trim().trim_end_matches('/').to_owned();
    let env = env.map(clean).filter(|value| !value.is_empty());
    for url in [clean(configured)]
        .into_iter()
        .chain(env)
        .chain([DEFAULT_DECIDER_BASE_URL.to_owned()])
        .chain(FALLBACKS.map(str::to_owned))
    {
        let url = url.as_str();
        if !list.iter().any(|known| known == url) {
            list.push(url.to_owned());
        }
    }
    list
}

/// A URL quer dizer "o Ollaya desta máquina"? O nome do serviço, o loopback
/// e o host do Docker; o IP ou nome de outra máquina, não.
fn means_this_machine(url: &str) -> bool {
    let url = url.trim();
    let parsed = if url.contains("://") {
        reqwest::Url::parse(url)
    } else {
        reqwest::Url::parse(&format!("http://{url}"))
    };
    parsed
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| {
            is_loopback_host(&host)
                || ["ollaya", DOCKER_HOST_ALIAS, "host.containers.internal"]
                    .iter()
                    .any(|name| host.eq_ignore_ascii_case(name))
        })
}

/// Procurar só quando a URL aponta para esta máquina — a de outra é soberana.
fn searches(settings: &AiDeciderSettings) -> bool {
    settings.provider == DeciderProvider::Ollaya && means_this_machine(&settings.base_url)
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

fn remembered(configured: &str) -> Option<String> {
    let guard = found().read().ok()?;
    guard
        .as_ref()
        .filter(|(asked, _, at)| asked == configured && at.elapsed() < REMEMBER)
        .map(|(_, url, _)| url.clone())
}

/// O primeiro endereço que responde, em paralelo. Nenhum respondendo, fica o
/// primeiro da lista — e a mensagem de erro mostra os que foram tentados.
async fn locate(configured: &str) -> Result<String, Vec<String>> {
    if let Some(url) = remembered(configured) {
        return Ok(url);
    }
    let list = candidates(configured, std::env::var(URL_ENV).ok().as_deref());
    // A da tela sozinha primeiro: respondendo, não espera a resolução de um
    // nome que não existe nesta rede (`ollaya` fora do compose).
    let url = if answers(&list[0]).await {
        list[0].clone()
    } else {
        let rest = &list[1..];
        let replies = join_all(rest.iter().map(|url| answers(url))).await;
        let Some(url) = rest
            .iter()
            .zip(replies)
            .find_map(|(url, ok)| ok.then(|| url.clone()))
        else {
            return Err(list);
        };
        url
    };
    if let Ok(mut guard) = found().write() {
        *guard = Some((configured.to_owned(), url.clone(), Instant::now()));
    }
    if url != configured.trim_end_matches('/') {
        tracing::info!(%url, configured, "decisor: Ollaya encontrado fora da URL da tela");
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
    match locate(&settings.base_url).await {
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
            candidates(DEFAULT_DECIDER_BASE_URL, Some(" http://127.0.0.1:11435/ ")),
            vec![
                "http://ollaya:11435",
                "http://127.0.0.1:11435",
                "http://host.docker.internal:11435",
            ]
        );
        assert_eq!(
            candidates(DEFAULT_DECIDER_BASE_URL, Some("  "))
                .first()
                .unwrap(),
            DEFAULT_DECIDER_BASE_URL
        );
    }

    #[test]
    fn a_url_da_tela_e_tentada_primeiro() {
        assert_eq!(
            candidates("http://localhost:12000/", None),
            vec![
                "http://localhost:12000",
                "http://ollaya:11435",
                "http://127.0.0.1:11435",
                "http://host.docker.internal:11435",
            ]
        );
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
