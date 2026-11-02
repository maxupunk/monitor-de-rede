//! Onde está um servidor de modelos do compose — o Ollaya (Decisor) ou o
//! Ollama (chat).
//!
//! A URL padrão (`http://ollaya:11435`, `http://ollama:11434`) é o nome do
//! serviço na rede interna do compose — e só vale ali. Com o override de rede
//! do host (`docker-compose.host.yml`, o que o syslog pede para ver o IP real
//! dos roteadores) a API sai dessa rede e o nome deixa de resolver, embora o
//! servidor esteja de pé. Instalações montadas à mão (um painel de
//! containers, outra rede, `cargo loco start` no host) caem no mesmo buraco.
//!
//! Por isso, enquanto a tela apontar para "esta máquina" — o nome do serviço,
//! o loopback (`127.0.0.1`, `localhost`) ou o host do Docker —, o servidor
//! procura nos endereços em que o compose o deixa: a URL da tela primeiro,
//! depois a variável da instalação (`OLLAYA_URL`, `OLLAMA_URL`; o override de
//! host as define), o nome do serviço, o loopback (porta publicada em
//! `127.0.0.1`) e o host do Docker. O loopback entra na procura porque o seu
//! sentido muda com a rede do container: na bridge, `127.0.0.1` é o próprio
//! container, e quem digitou quis dizer "esta máquina". O endereço de outra
//! máquina é escolha do operador e nunca é trocado.

use std::{
    sync::{OnceLock, RwLock},
    time::{Duration, Instant},
};

use futures::future::join_all;

use crate::services::shared::runtime::{is_loopback_host, DOCKER_HOST_ALIAS};

/// Quanto esperar cada endereço. Nome que não resolve e porta fechada falham
/// na hora; o teto só pesa num endereço que engole pacotes.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1_500);

/// Quanto tempo um endereço achado vale sem nova procura.
const REMEMBER: Duration = Duration::from_secs(5 * 60);

/// O último achado: a URL da tela, onde o servidor respondeu e quando.
type Found = Option<(String, String, Instant)>;

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Um servidor de modelos que o compose sobe ao lado da API.
pub struct LocalServer {
    /// Nome nos logs ("Ollaya", "Ollama").
    label: &'static str,
    /// Nome do serviço no compose — o host da URL padrão.
    service: &'static str,
    port: u16,
    /// Variável com o endereço definido pela instalação — vem antes dos palpites.
    env: &'static str,
    found: RwLock<Found>,
}

impl LocalServer {
    #[must_use]
    pub const fn new(
        label: &'static str,
        service: &'static str,
        port: u16,
        env: &'static str,
    ) -> Self {
        Self {
            label,
            service,
            port,
            env,
            found: RwLock::new(None),
        }
    }

    /// A URL do serviço na rede interna do compose.
    #[must_use]
    pub fn default_url(&self) -> String {
        format!("http://{}:{}", self.service, self.port)
    }

    /// Os endereços a tentar, na ordem, sem repetição: o da tela, o da
    /// instalação e os palpites.
    fn candidates(&self, configured: &str, env: Option<&str>) -> Vec<String> {
        let mut list: Vec<String> = Vec::new();
        let clean = |value: &str| value.trim().trim_end_matches('/').to_owned();
        let env = env.map(clean).filter(|value| !value.is_empty());
        let fallbacks = [
            format!("http://127.0.0.1:{}", self.port),
            format!("http://{DOCKER_HOST_ALIAS}:{}", self.port),
        ];
        for url in [clean(configured)]
            .into_iter()
            .chain(env)
            .chain([self.default_url()])
            .chain(fallbacks)
        {
            if !list.contains(&url) {
                list.push(url);
            }
        }
        list
    }

    /// A URL quer dizer "o servidor desta máquina"? O nome do serviço, o
    /// loopback e o host do Docker; o IP ou nome de outra máquina, não.
    #[must_use]
    pub fn means_this_machine(&self, url: &str) -> bool {
        let url = url.trim();
        let parsed = if url.contains("://") {
            reqwest::Url::parse(url)
        } else {
            reqwest::Url::parse(&format!("http://{url}"))
        };
        parsed.ok().is_some_and(|url| {
            url.port().is_none_or(|port| port == self.port)
                && url.host_str().is_some_and(|host| {
                    is_loopback_host(host)
                        || [self.service, DOCKER_HOST_ALIAS, "host.containers.internal"]
                            .iter()
                            .any(|name| host.eq_ignore_ascii_case(name))
                })
        })
    }

    fn remembered(&self, configured: &str) -> Option<String> {
        let guard = self.found.read().ok()?;
        guard
            .as_ref()
            .filter(|(asked, _, at)| asked == configured && at.elapsed() < REMEMBER)
            .map(|(_, url, _)| url.clone())
    }

    /// O primeiro endereço que responde, em paralelo. Nenhum respondendo, a
    /// lista dos tentados, para a mensagem de erro.
    ///
    /// `configured` é a raiz do servidor (sem `/v1`).
    ///
    /// # Errors
    ///
    /// Os endereços tentados, quando nenhum respondeu.
    pub async fn locate(&self, configured: &str) -> Result<String, Vec<String>> {
        if let Some(url) = self.remembered(configured) {
            return Ok(url);
        }
        let list = self.candidates(configured, std::env::var(self.env).ok().as_deref());
        // A da tela sozinha primeiro: respondendo, não espera a resolução de
        // um nome que não existe nesta rede (o serviço fora do compose).
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
        if let Ok(mut guard) = self.found.write() {
            *guard = Some((configured.to_owned(), url.clone(), Instant::now()));
        }
        if url != configured.trim_end_matches('/') {
            tracing::info!(%url, configured, server = self.label, "servidor de modelos encontrado fora da URL da tela");
        }
        Ok(url)
    }
}

/// Qualquer resposta HTTP conta, até um 401: quem respondeu é o servidor, e
/// a chave é outro problema, com a sua própria mensagem.
async fn answers(url: &str) -> bool {
    http()
        .get(format!("{url}/api/tags"))
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    static OLLAYA: LocalServer = LocalServer::new("Ollaya", "ollaya", 11435, "OLLAYA_URL");

    #[test]
    fn a_variavel_vem_antes_e_nada_se_repete() {
        assert_eq!(
            OLLAYA.candidates("http://ollaya:11435", Some(" http://127.0.0.1:11435/ ")),
            vec![
                "http://ollaya:11435",
                "http://127.0.0.1:11435",
                "http://host.docker.internal:11435",
            ]
        );
        assert_eq!(
            OLLAYA
                .candidates("http://ollaya:11435", Some("  "))
                .first()
                .unwrap(),
            "http://ollaya:11435"
        );
    }

    #[test]
    fn a_url_da_tela_e_tentada_primeiro() {
        assert_eq!(
            OLLAYA.candidates("http://localhost:12000/", None),
            vec![
                "http://localhost:12000",
                "http://ollaya:11435",
                "http://127.0.0.1:11435",
                "http://host.docker.internal:11435",
            ]
        );
    }

    #[test]
    fn esta_maquina_e_o_servico_o_loopback_e_o_host_do_docker() {
        for local in [
            "http://ollaya:11435",
            "http://127.0.0.1:11435",
            "localhost:11435",
            "http://[::1]:11435",
            "http://host.docker.internal:11435",
        ] {
            assert!(OLLAYA.means_this_machine(local), "{local}");
        }
        assert!(!OLLAYA.means_this_machine("http://10.0.0.5:11435"));
        // O nome do serviço e a porta são do servidor: `ollama` ou outra porta local não são o Ollaya.
        assert!(!OLLAYA.means_this_machine("http://ollama:11434"));
        assert!(!OLLAYA.means_this_machine("http://127.0.0.1:9"));
    }
}
