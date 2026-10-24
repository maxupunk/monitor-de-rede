//! Modelos de decisão para a tela escolher: os medidos no benchmark das
//! decisões do NetMonitor ([`decisions_benchmark`], gerado pelo Laya-tuning),
//! com a ficha de cada um, e os que o Ollaya já tem instalados.
//!
//! Só entra aqui modelo que passou por esse benchmark — as perguntas reais do
//! NetMonitor, em português, na CPU de uma instalação comum. Nota, latência e
//! tempo sugerido por pergunta saem dele; nada de número publicado por fora
//! (autor do modelo, Ollaya ou testes públicos), que mede outras perguntas em
//! outro hardware.
//!
//! A ficha ([`ModelSpec`]) guarda só o que não se mede: família, língua,
//! tamanho do download e se vem com o NetMonitor. O resto — tempo sugerido,
//! prazo de carregamento — sai de regras únicas aqui, sem número solto por
//! modelo.

use std::{cmp::Reverse, time::Duration};

use super::{
    client::DeciderClient,
    config::{AiDeciderSettings, DEFAULT_DECIDER_TIMEOUT_MS},
    decisions_benchmark,
};
use crate::{
    dtos::ai::{
        DeciderModelLanguage::{self, English, Multilingual},
        DeciderModelOption, DeciderModelSpecView, DeciderModelsResponse,
    },
    services::ai::local_models,
};

/// Prazo de carregamento de um modelo fora do catálogo.
pub const DEFAULT_LOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// A ficha de um modelo conhecido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: &'static str,
    /// Outros nomes do mesmo modelo no Ollaya.
    pub aliases: &'static [&'static str],
    pub family: &'static str,
    pub description: &'static str,
    pub language: DeciderModelLanguage,
    pub download_mb: Option<u32>,
    /// Recomendado para o NetMonitor.
    pub recommended: bool,
    /// Vem instalado com o NetMonitor (serviço `ollaya-models` do compose): não
    /// está no registro do Ollaya, então não se baixa pela tela.
    pub bundled: bool,
}

/// O que não muda de um modelo para outro; cada entrada diz só o que é dela.
const BASE: ModelSpec = ModelSpec {
    name: "",
    aliases: &[],
    family: "",
    description: "",
    language: English,
    download_mb: None,
    recommended: false,
    bundled: false,
};

/// Faixa do "tempo máximo por pergunta" da configuração.
const MIN_TIMEOUT_MS: u32 = 1_500;
const MAX_TIMEOUT_MS: u32 = 10_000;

impl ModelSpec {
    /// Tempo máximo por pergunta sugerido: o dobro do p95 medido no benchmark
    /// das decisões (em CPU), arredondado para cima de 500 em 500 ms, dentro da
    /// faixa da configuração. Sem medição, o padrão da configuração.
    #[must_use]
    pub fn suggested_timeout_ms(&self) -> u32 {
        decisions_benchmark::result(self.name).map_or(DEFAULT_DECIDER_TIMEOUT_MS, |result| {
            // Limitado à faixa da configuração: cabe com folga num u32.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let steps = (result.p95_ms * 2.0 / 500.0).ceil().max(0.0) as u32;
            steps
                .saturating_mul(500)
                .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
        })
    }

    /// Tirar o modelo do disco: 30 s por GB baixado, de 2 a 15 minutos — um
    /// encoder pequeno sai em 5 a 20 s.
    #[must_use]
    pub fn load_timeout(&self) -> Duration {
        let secs = self.download_mb.map_or(0, |mb| u64::from(mb) * 30 / 1_000);
        Duration::from_secs(secs.clamp(120, 900))
    }

    /// A nota no benchmark das decisões do NetMonitor, em milésimos.
    #[must_use]
    pub fn score_permille(&self) -> Option<u16> {
        decisions_benchmark::result(self.name).map(decisions_benchmark::ModelResult::permille)
    }

    /// Posição na lista: o recomendado primeiro, depois da maior nota para a
    /// menor. Empate fica com o menor download.
    #[must_use]
    pub fn rank(&self) -> (Reverse<bool>, Reverse<u16>, u32) {
        (
            Reverse(self.recommended),
            Reverse(self.score_permille().unwrap_or(0)),
            self.download_mb.unwrap_or(u32::MAX),
        )
    }

    fn answers_to(&self, name: &str) -> bool {
        same_tag(self.name, name) || self.aliases.iter().any(|alias| same_tag(alias, name))
    }

    fn view(&self) -> DeciderModelSpecView {
        DeciderModelSpecView {
            family: self.family.to_string(),
            language: self.language,
            download_mb: self.download_mb,
            suggested_timeout_ms: self.suggested_timeout_ms(),
            recommended: self.recommended,
            bundled: self.bundled,
        }
    }
}

/// Os modelos medidos no benchmark das decisões do NetMonitor.
pub const KNOWN_MODELS: &[ModelSpec] = &[
    // ── NetMonitor: ajustado nas decisões dele, o padrão ──
    ModelSpec {
        name: "netmonitor/decima-network:v11",
        aliases: &["decima-network", "netmonitor/decima-network"],
        family: "Decima Network",
        description: "Decima Network (NetMonitor): o decima:small ajustado nas decisões de rede do NetMonitor, em português — o mais preciso no benchmark das decisões e o mais rápido em CPU. Vem instalado com o NetMonitor",
        language: Multilingual,
        download_mb: Some(510),
        recommended: true,
        bundled: true,
    },
    // ── Em inglês, do registro do Ollaya ──
    ModelSpec {
        name: "kev:0.8b",
        family: "Kev",
        description: "Kev (Jared Palmer): Qwen3.5 0,8B, só inglês; lento em CPU — boa parte das perguntas passa do tempo máximo",
        download_mb: Some(1_800),
        ..BASE
    },
    ModelSpec {
        name: "decider:0.8b",
        family: "Decider",
        description: "Decider (Mapika): Qwen3.5 0,8B, só inglês; lento em CPU — boa parte das perguntas passa do tempo máximo",
        download_mb: Some(1_500),
        ..BASE
    },
];

/// O mesmo modelo: sem caixa e com `:latest` implícito.
fn same_tag(a: &str, b: &str) -> bool {
    let normalize = |tag: &str| {
        let tag = tag.trim().to_lowercase();
        tag.strip_suffix(":latest")
            .map(str::to_string)
            .unwrap_or(tag)
    };
    normalize(a) == normalize(b)
}

/// O que fazer quando falta um modelo que vem com o NetMonitor.
pub const BUNDLED_HINT: &str = "Ele vem com o NetMonitor e é instalado pelo serviço `ollaya-models`: rode `docker compose up -d ollaya-models` no servidor.";

/// Vem instalado com o NetMonitor: não está no registro do Ollaya.
#[must_use]
pub fn is_bundled(model: &str) -> bool {
    spec(model).is_some_and(|spec| spec.bundled)
}

/// A ficha do modelo, pelo nome ou por um apelido.
#[must_use]
pub fn spec(model: &str) -> Option<&'static ModelSpec> {
    KNOWN_MODELS.iter().find(|spec| spec.answers_to(model))
}

/// Quanto esperar o Ollaya tirar o modelo do disco.
#[must_use]
pub fn load_timeout(model: &str) -> Duration {
    spec(model).map_or(DEFAULT_LOAD_TIMEOUT, ModelSpec::load_timeout)
}

/// O modelo está na lista do Ollaya (instalados ou carregados), por qualquer
/// nome dele: `netmonitor/decima-network:v11` vale para quem pediu
/// `decima-network`. O apelido é comparado exato — `kev:0.8b` não casa com um
/// `kev:4b` instalado. Fora do catálogo, vale a regra de nomes
/// (`local_models::is_installed`).
#[must_use]
pub fn is_available<S: AsRef<str>>(names: &[S], model: &str) -> bool {
    match spec(model) {
        Some(spec) => names.iter().any(|name| spec.answers_to(name.as_ref())),
        None => local_models::is_installed(names, model),
    }
}

/// Os modelos de um provedor de nuvem: os do preset, prontos para uso — e o
/// digitado, quando é outro. Não vai à rede: a chave se confere no teste.
fn cloud_models(settings: &AiDeciderSettings) -> DeciderModelsResponse {
    let mut options: Vec<DeciderModelOption> = settings
        .preset()
        .models
        .iter()
        .map(|model| DeciderModelOption {
            name: model.name.to_string(),
            description: Some(model.description.to_string()),
            installed: true,
            spec: None,
            decisions: None,
        })
        .collect();
    if !settings.model.is_empty() && !options.iter().any(|option| option.name == settings.model) {
        options.push(DeciderModelOption {
            name: settings.model.clone(),
            description: None,
            installed: true,
            spec: None,
            decisions: None,
        });
    }
    DeciderModelsResponse {
        online: true,
        manages_models: false,
        error_message: None,
        installed: Vec::new(),
        loaded: Vec::new(),
        options,
    }
}

/// Nunca falha: fora do ar vira `online: false` com o motivo.
pub async fn list(settings: &AiDeciderSettings) -> DeciderModelsResponse {
    if !settings.manages_models() {
        return cloud_models(settings);
    }
    let client = DeciderClient::connect(settings).await;
    let (installed, error_message) = match client.installed_models().await {
        Ok(installed) => (Some(installed), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let online = installed.is_some();
    // Na memória agora; falha aqui (Ollaya antigo) só deixa a lista vazia.
    let loaded = if online {
        client.loaded_models().await.unwrap_or_default()
    } else {
        Vec::new()
    };
    let installed = installed.unwrap_or_default();

    let mut options: Vec<DeciderModelOption> = KNOWN_MODELS
        .iter()
        .map(|spec| DeciderModelOption {
            name: spec.name.to_string(),
            description: Some(spec.description.to_string()),
            installed: is_available(&installed, spec.name),
            spec: Some(spec.view()),
            decisions: decisions_benchmark::result(spec.name)
                .map(decisions_benchmark::ModelResult::score_view),
        })
        .collect();
    options.sort_by_key(|option| spec(&option.name).map(ModelSpec::rank));
    // Instalado fora do catálogo (um modelo próprio, uma tag nova) também aparece.
    for name in &installed {
        if spec(name).is_none() {
            options.push(DeciderModelOption {
                name: name.clone(),
                description: None,
                installed: true,
                spec: None,
                decisions: decisions_benchmark::result(name)
                    .map(decisions_benchmark::ModelResult::score_view),
            });
        }
    }

    DeciderModelsResponse {
        online,
        manages_models: true,
        error_message,
        installed,
        loaded,
        options,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::services::ai::decider::config::DEFAULT_DECIDER_MODEL;

    #[tokio::test]
    async fn fora_do_ar_ainda_oferece_o_catalogo() {
        let settings = AiDeciderSettings {
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 500,
            ..AiDeciderSettings::default()
        };
        let models = list(&settings).await;
        assert!(!models.online);
        assert!(models.error_message.is_some());
        assert_eq!(models.options.len(), KNOWN_MODELS.len());
        assert!(models.options.iter().all(|option| !option.installed));
        assert!(models.options.iter().all(|option| option.spec.is_some()));
        assert!(models
            .options
            .iter()
            .all(|option| option.decisions.is_some()));
    }

    #[tokio::test]
    async fn nuvem_lista_os_modelos_do_preset_sem_ir_a_rede() {
        let settings = AiDeciderSettings {
            provider: crate::services::ai::decider::providers::DeciderProvider::Cloudflare,
            base_url: "http://127.0.0.1:9".into(),
            model: "clef".into(),
            ..AiDeciderSettings::default()
        };
        let models = list(&settings).await;
        assert!(models.online && !models.manages_models);
        let nomes: Vec<&str> = models
            .options
            .iter()
            .map(|option| option.name.as_str())
            .collect();
        assert_eq!(nomes, ["clef-flash", "clef"]);
        assert!(models.options.iter().all(|option| option.installed));
    }

    #[test]
    fn padrao_e_o_decima_network_v11_e_vem_instalado() {
        let padrao = spec(DEFAULT_DECIDER_MODEL).unwrap();
        assert_eq!(padrao.name, "netmonitor/decima-network:v11");
        assert!(padrao.recommended && padrao.bundled);
        assert!(
            padrao
                .score_permille()
                .is_some_and(|permille| permille >= 900),
            "nota do benchmark das decisões do NetMonitor"
        );
        assert!(
            padrao.suggested_timeout_ms() <= DEFAULT_DECIDER_TIMEOUT_MS,
            "o padrão cabe no tempo padrão da configuração"
        );
    }

    #[test]
    fn um_so_recomendado() {
        let recomendados: Vec<&str> = KNOWN_MODELS
            .iter()
            .filter(|spec| spec.recommended)
            .map(|spec| spec.name)
            .collect();
        assert_eq!(recomendados, [DEFAULT_DECIDER_MODEL]);
    }

    #[test]
    fn lista_abre_no_recomendado_e_segue_pela_nota() {
        let mut ordem: Vec<&ModelSpec> = KNOWN_MODELS.iter().collect();
        ordem.sort_by_key(|spec| spec.rank());
        let nomes: Vec<&str> = ordem.iter().map(|spec| spec.name).collect();
        assert_eq!(
            nomes,
            ["netmonitor/decima-network:v11", "kev:0.8b", "decider:0.8b"]
        );
    }

    #[test]
    fn nomes_e_apelidos_sao_unicos() {
        let mut seen = HashSet::new();
        for spec in KNOWN_MODELS {
            for name in std::iter::once(&spec.name).chain(spec.aliases) {
                assert!(seen.insert(name.to_lowercase()), "{name} repetido");
            }
        }
    }

    #[test]
    fn todo_modelo_do_catalogo_foi_medido_no_benchmark_do_netmonitor() {
        for spec in KNOWN_MODELS {
            assert!(
                decisions_benchmark::result(spec.name).is_some(),
                "{} sem medição",
                spec.name
            );
            assert!(!spec.description.is_empty(), "{}", spec.name);
            assert!(!spec.family.is_empty(), "{}", spec.name);
        }
    }

    #[test]
    fn apelido_resolve_para_o_modelo() {
        assert_eq!(
            spec("decima-network").map(|spec| spec.name),
            Some("netmonitor/decima-network:v11")
        );
        assert_eq!(spec("KEV:0.8b").map(|spec| spec.name), Some("kev:0.8b"));
        assert!(spec("kev").is_none(), "o kev sem tag é outro tamanho");
        assert!(spec("modelo-proprio").is_none());
    }

    #[test]
    fn disponivel_pelo_apelido_sem_confundir_tamanhos() {
        assert!(is_available(&["kev:0.8b"], "kev:0.8b"));
        assert!(!is_available(&["kev:4b"], "kev:0.8b"), "o 4B não é o 0.8B");
        assert!(!is_available(&["kev:latest"], "kev:0.8b"));
        assert!(is_available(
            &["netmonitor/decima-network:v11"],
            "decima-network"
        ));
        assert!(
            !is_available(&["netmonitor/decima-network:v9"], DEFAULT_DECIDER_MODEL),
            "a v9 não é a v11"
        );
        assert!(is_available(&["laya:latest"], "laya"), "fora do catálogo");
        assert!(is_available(&["meu-modelo"], "meu-modelo"));
    }

    #[test]
    fn tempo_sugerido_e_o_dobro_do_p95_medido() {
        let tempo = |name: &str| spec(name).unwrap().suggested_timeout_ms();
        assert_eq!(tempo(DEFAULT_DECIDER_MODEL), 2_000, "p95 de 981 ms × 2");
        assert_eq!(tempo("kev:0.8b"), MAX_TIMEOUT_MS, "o teto da configuração");
        assert_eq!(tempo("decider:0.8b"), MAX_TIMEOUT_MS);
        let fora = ModelSpec {
            name: "modelo-proprio",
            ..BASE
        };
        assert_eq!(fora.suggested_timeout_ms(), DEFAULT_DECIDER_TIMEOUT_MS);
    }

    #[test]
    fn modelo_grande_tem_prazo_maior_para_carregar() {
        let com_download = |mb: u32| ModelSpec {
            download_mb: Some(mb),
            ..BASE
        };
        assert_eq!(
            com_download(19_500).load_timeout(),
            Duration::from_secs(585)
        );
        assert_eq!(
            com_download(40_000).load_timeout(),
            Duration::from_secs(900)
        );
        assert_eq!(load_timeout("kev:0.8b"), Duration::from_secs(120), "o piso");
        assert_eq!(load_timeout("decima-network"), Duration::from_secs(120));
        assert_eq!(load_timeout("desconhecido"), DEFAULT_LOAD_TIMEOUT);
    }
}
