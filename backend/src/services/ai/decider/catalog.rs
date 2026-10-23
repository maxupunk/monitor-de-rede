//! Modelos de decisão para a tela escolher: os do Ollama que respondem às
//! perguntas do NetMonitor, com a ficha de cada um, e os que o Ollama já tem
//! instalados.
//!
//! **Compatível** quer dizer: responde pelo `POST /v1/systemone` aos dois
//! tipos que as decisões usam — `noul` (sim/não) e `choice` (escolha) —, roda
//! no Ollama de um servidor Linux (GGUF; tags só MLX, de Mac, ficam de fora,
//! como o `strands-decider`) e cabe numa instalação comum. Ficam de fora
//! também os de 9B ou mais (`nimble-v3`, `pplx-decider`, `clef`): pesados para
//! o servidor que roda o NetMonitor, e o `nimble-v3` ainda tem licença só para
//! uso não comercial.
//!
//! A ficha ([`ModelSpec`]) guarda só o que foi medido: o download no registro
//! do Ollama e, quando houve rodada em CPU, a latência. A nota que ordena e
//! recomenda vem do benchmark das decisões do NetMonitor
//! (`decider::decisions_benchmark`, rodadas do Laya-tuning); sem medição em
//! CPU, a latência fica em branco — nunca um número de outro hardware.

use std::{cmp::Reverse, time::Duration};

use super::{
    benchmark::USEFUL_PERMILLE, client::DeciderClient, config::AiDeciderSettings,
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

/// Onde o modelo deve rodar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hardware {
    /// Encoder pequeno: roda bem sem placa de vídeo.
    Cpu,
    /// Decodificador (Qwen) de bilhões de parâmetros: em CPU, segundos por
    /// pergunta.
    Gpu,
}

/// A ficha de um modelo conhecido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: &'static str,
    /// Outros nomes do mesmo modelo no Ollama.
    pub aliases: &'static [&'static str],
    pub family: &'static str,
    pub description: &'static str,
    pub language: DeciderModelLanguage,
    pub download_mb: Option<u32>,
    /// Memória com o modelo carregado, quando medida.
    pub memory_mb: Option<u32>,
    /// Mediana de uma decisão em CPU no benchmark do NetMonitor; `None`
    /// quando não houve rodada em CPU.
    pub latency_ms: Option<u32>,
    pub hardware: Hardware,
    /// Teto de opções por escolha, quando o modelo tem um.
    pub max_choice_options: Option<u16>,
    /// Recomendado para o NetMonitor.
    pub recommended: bool,
    /// Vem com o NetMonitor: não está no registro do Ollama, e a própria API
    /// o instala a partir da release do projeto (`decider::bundled`).
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
    memory_mb: None,
    latency_ms: None,
    hardware: Hardware::Cpu,
    max_choice_options: None,
    recommended: false,
    bundled: false,
};

/// Faixa do "tempo máximo por pergunta" da configuração.
const MIN_TIMEOUT_MS: u32 = 1_500;
const MAX_TIMEOUT_MS: u32 = 10_000;
/// Sem medição em CPU: o padrão da configuração.
const UNMEASURED_CPU_TIMEOUT_MS: u32 = super::config::DEFAULT_DECIDER_TIMEOUT_MS;

/// Opções por escolha que a Laya aceita (página do modelo no Ollama).
const LAYA_MAX_OPTIONS: u16 = 26;

impl ModelSpec {
    #[must_use]
    pub const fn gpu_recommended(&self) -> bool {
        matches!(self.hardware, Hardware::Gpu)
    }

    /// Tempo máximo por pergunta sugerido: o p95 medido em CPU no benchmark
    /// do NetMonitor com 10% de folga; sem ele, a mediana × 3; sem medição
    /// nenhuma, o padrão da configuração (CPU) ou o teto (GPU). Arredonda
    /// para cima, de 500 em 500 ms, dentro da faixa da configuração.
    #[must_use]
    pub fn suggested_timeout_ms(&self) -> u32 {
        let measured_p95 = decisions_benchmark::result(self.name).and_then(|row| row.p95_ms);
        let raw = match (measured_p95, self.latency_ms, self.hardware) {
            // Milissegundos de uma decisão: cabe com folga num u32.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            (Some(p95), _, _) => (p95 * 1.1).ceil() as u32,
            (None, Some(median), _) => median * 3,
            (None, None, Hardware::Cpu) => UNMEASURED_CPU_TIMEOUT_MS,
            (None, None, Hardware::Gpu) => MAX_TIMEOUT_MS,
        };
        (raw.div_ceil(500) * 500).clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
    }

    /// Tirar o modelo do disco: 30 s por GB baixado, de 2 a 15 minutos.
    #[must_use]
    pub fn load_timeout(&self) -> Duration {
        let secs = self.download_mb.map_or(0, |mb| u64::from(mb) * 30 / 1_000);
        Duration::from_secs(secs.clamp(120, 900))
    }

    /// A nota que recomenda e ordena, em milésimos: a do benchmark das
    /// decisões do NetMonitor.
    #[must_use]
    pub fn score_permille(&self) -> Option<u16> {
        decisions_benchmark::result(self.name).map(decisions_benchmark::ModelResult::permille)
    }

    /// Posição na lista: primeiro os aprovados (nota acima do chute), da
    /// maior nota para a menor; depois os não avaliados; por fim os que ficaram
    /// perto do chute. Empate fica com o menor download.
    #[must_use]
    pub fn rank(&self) -> (u8, Reverse<u16>, u32) {
        let score = self.score_permille();
        let tier = match score {
            Some(permille) if permille >= USEFUL_PERMILLE => 0,
            None => 1,
            Some(_) => 2,
        };
        (
            tier,
            Reverse(score.unwrap_or(0)),
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
            memory_mb: self.memory_mb,
            latency_ms: self.latency_ms,
            gpu_recommended: self.gpu_recommended(),
            max_choice_options: self.max_choice_options,
            suggested_timeout_ms: self.suggested_timeout_ms(),
            recommended: self.recommended,
            bundled: self.bundled,
        }
    }
}

/// Os modelos compatíveis: primeiro o ajustado do NetMonitor, depois os Laya
/// da biblioteca (encoders, CPU), por fim os decodificadores (GPU).
pub const KNOWN_MODELS: &[ModelSpec] = &[
    ModelSpec {
        name: "netmonitor/laya-network:v2",
        aliases: &["laya-network", "netmonitor/laya-network"],
        family: "Laya Network",
        description: "Laya ajustada nas decisões do NetMonitor (mmBERT 322M, 100+ línguas): a melhor nota no benchmark delas e 80% em casos reais nunca vistos. Vem com o NetMonitor — a API a instala no Ollama",
        language: Multilingual,
        download_mb: Some(658),
        latency_ms: Some(1_860),
        max_choice_options: Some(LAYA_MAX_OPTIONS),
        recommended: true,
        bundled: true,
        ..BASE
    },
    // ── Laya (Convai Innovations): encoders, sem ajuste ──
    ModelSpec {
        name: "laya:322m-multilingual-bf16",
        family: "Laya",
        description: "Laya: mmBERT 322M, 100+ línguas — a base do Laya Network, perto do chute sem o ajuste",
        language: Multilingual,
        download_mb: Some(658),
        max_choice_options: Some(LAYA_MAX_OPTIONS),
        ..BASE
    },
    ModelSpec {
        name: "laya:421m-english-bf16",
        aliases: &["laya"],
        family: "Laya",
        description: "Laya: ModernBERT-large 421M, só inglês",
        download_mb: Some(844),
        max_choice_options: Some(LAYA_MAX_OPTIONS),
        ..BASE
    },
    ModelSpec {
        name: "laya:421m-typed-decisions-bf16",
        family: "Laya",
        description: "Laya: ModernBERT-large ajustado nas decisões tipadas, só inglês",
        download_mb: Some(844),
        max_choice_options: Some(LAYA_MAX_OPTIONS),
        ..BASE
    },
    // ── Decodificadores (Qwen + cabeça de decisão): pedem GPU ──
    ModelSpec {
        name: "tev1:4b-q4_K_M",
        family: "Tev",
        description: "Tev 4B (Together AI), quantizado em 4 bits",
        download_mb: Some(2_783),
        hardware: Hardware::Gpu,
        ..BASE
    },
    ModelSpec {
        name: "kev:4b-q4_K_M",
        family: "Kev",
        description: "Kev 4B (Jared Palmer), quantizado em 4 bits",
        download_mb: Some(2_710),
        hardware: Hardware::Gpu,
        ..BASE
    },
    ModelSpec {
        name: "d3:2b",
        family: "D3",
        description: "D3 2B: decide sobre texto e imagem",
        download_mb: Some(3_224),
        hardware: Hardware::Gpu,
        ..BASE
    },
    ModelSpec {
        name: "kev:0.8b-q8_0",
        family: "Kev",
        description: "Kev 0,8B (Jared Palmer), o menor da família",
        download_mb: Some(812),
        hardware: Hardware::Gpu,
        ..BASE
    },
    ModelSpec {
        name: "tev1:0.8b-q8_0",
        family: "Tev",
        description: "Tev 0,8B (Together AI), o menor da família",
        download_mb: Some(834),
        hardware: Hardware::Gpu,
        ..BASE
    },
];

/// O mesmo modelo: sem caixa e com `:latest` implícito.
pub(super) fn same_tag(a: &str, b: &str) -> bool {
    let normalize = |tag: &str| {
        let tag = tag.trim().to_lowercase();
        tag.strip_suffix(":latest")
            .map(str::to_string)
            .unwrap_or(tag)
    };
    normalize(a) == normalize(b)
}

/// O que fazer quando falta um modelo que vem com o NetMonitor.
pub const BUNDLED_HINT: &str = "Ele vem com o NetMonitor: use \"Baixar modelo\" (ou ligue o Decisor) e a API o instala no Ollama a partir da release do projeto.";

/// Vem com o NetMonitor: não está no registro do Ollama.
#[must_use]
pub fn is_bundled(model: &str) -> bool {
    spec(model).is_some_and(|spec| spec.bundled)
}

/// A ficha do modelo, pelo nome ou por um apelido.
#[must_use]
pub fn spec(model: &str) -> Option<&'static ModelSpec> {
    KNOWN_MODELS.iter().find(|spec| spec.answers_to(model))
}

/// Quanto esperar o Ollama tirar o modelo do disco.
#[must_use]
pub fn load_timeout(model: &str) -> Duration {
    spec(model).map_or(DEFAULT_LOAD_TIMEOUT, ModelSpec::load_timeout)
}

/// Teto de opções por escolha do modelo; `None` quando ele não tem um.
#[must_use]
pub fn max_choice_options(model: &str) -> Option<usize> {
    spec(model)
        .and_then(|spec| spec.max_choice_options)
        .map(usize::from)
}

/// O modelo está na lista do Ollama (instalados ou carregados), por qualquer
/// nome dele; fora do catálogo, vale a regra de `local_models::is_installed`.
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
            benchmark: model.benchmark.map(|benchmark| benchmark.view()),
            decisions: None,
        })
        .collect();
    if !settings.model.is_empty() && !options.iter().any(|option| option.name == settings.model) {
        options.push(DeciderModelOption {
            name: settings.model.clone(),
            description: None,
            installed: true,
            spec: None,
            benchmark: None,
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
    // Na memória agora; falha aqui só deixa a lista vazia.
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
            benchmark: None,
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
                benchmark: None,
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
    use crate::services::devices::kinds::DEVICE_KINDS;

    #[tokio::test]
    async fn fora_do_ar_ainda_oferece_o_catalogo() {
        let settings = AiDeciderSettings {
            // Endereço de outra máquina: a procura do `locate` não entra.
            base_url: "http://192.0.2.1:9".into(),
            timeout_ms: 500,
            ..AiDeciderSettings::default()
        };
        let models = list(&settings).await;
        assert!(!models.online);
        assert!(models.error_message.is_some());
        assert_eq!(models.options.len(), KNOWN_MODELS.len());
        assert!(models.options.iter().all(|option| !option.installed));
        assert!(models.options.iter().all(|option| option.spec.is_some()));
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
    fn padrao_e_o_laya_network_em_cpu_e_vem_com_o_netmonitor() {
        let padrao = spec(super::super::config::DEFAULT_DECIDER_MODEL).unwrap();
        assert_eq!(padrao.name, "netmonitor/laya-network:v2");
        assert!(padrao.recommended && padrao.bundled);
        assert!(!padrao.gpu_recommended());
        assert!(
            padrao
                .score_permille()
                .is_some_and(|permille| permille >= 850),
            "nota do benchmark das decisões do NetMonitor"
        );
        assert!(
            padrao.suggested_timeout_ms() <= super::super::config::DEFAULT_DECIDER_TIMEOUT_MS,
            "o padrão cabe no tempo padrão da configuração"
        );
    }

    #[test]
    fn recomendado_tem_nota_alta_e_atende_portugues() {
        let recomendados: Vec<&ModelSpec> = KNOWN_MODELS
            .iter()
            .filter(|spec| spec.recommended)
            .collect();
        assert!(!recomendados.is_empty());
        for spec in recomendados {
            assert!(
                spec.score_permille()
                    .is_some_and(|permille| permille >= 850),
                "{}",
                spec.name
            );
            assert_eq!(spec.language, Multilingual, "{}", spec.name);
        }
    }

    #[test]
    fn lista_vai_da_maior_nota_a_menor_e_os_perto_do_chute_no_fim() {
        let mut ordem: Vec<&ModelSpec> = KNOWN_MODELS.iter().collect();
        ordem.sort_by_key(|spec| spec.rank());
        let nomes: Vec<&str> = ordem.iter().map(|spec| spec.name).collect();
        assert_eq!(
            nomes[0], "netmonitor/laya-network:v2",
            "o recomendado abre a lista"
        );
        let aprovados: Vec<u16> = ordem
            .iter()
            .filter_map(|spec| spec.score_permille())
            .take_while(|permille| *permille >= USEFUL_PERMILLE)
            .collect();
        assert!(
            aprovados.windows(2).all(|par| par[0] >= par[1]),
            "aprovados da maior nota para a menor: {nomes:?}"
        );
        if let Some(primeiro) = ordem
            .iter()
            .position(|spec| spec.score_permille().is_some_and(|p| p < USEFUL_PERMILLE))
        {
            assert!(
                ordem[primeiro..]
                    .iter()
                    .all(|spec| spec.score_permille().is_some_and(|p| p < USEFUL_PERMILLE)),
                "os perto do chute vão para o fim: {nomes:?}"
            );
        }
    }

    #[test]
    fn todo_modelo_do_catalogo_foi_medido_no_benchmark_do_netmonitor() {
        for spec in KNOWN_MODELS {
            assert!(spec.score_permille().is_some(), "{} sem nota", spec.name);
        }
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
    fn todo_modelo_aceita_a_maior_escolha_fixa() {
        for spec in KNOWN_MODELS {
            if let Some(max) = spec.max_choice_options {
                assert!(
                    usize::from(max) >= DEVICE_KINDS.len(),
                    "{} não escolhe entre os {} tipos de dispositivo",
                    spec.name,
                    DEVICE_KINDS.len()
                );
            }
        }
    }

    #[test]
    fn toda_ficha_tem_descricao_e_familia() {
        for spec in KNOWN_MODELS {
            assert!(!spec.description.is_empty(), "{}", spec.name);
            assert!(!spec.family.is_empty(), "{}", spec.name);
        }
    }

    #[test]
    fn apelido_resolve_para_o_modelo() {
        assert_eq!(
            spec("laya-network").map(|spec| spec.name),
            Some("netmonitor/laya-network:v2")
        );
        assert_eq!(
            spec("LAYA:latest").map(|spec| spec.name),
            Some("laya:421m-english-bf16")
        );
        assert!(spec("modelo-proprio").is_none());
    }

    #[test]
    fn disponivel_pelo_nome_ou_apelido() {
        assert!(is_available(&["laya:latest"], "laya:421m-english-bf16"));
        assert!(is_available(
            &["netmonitor/laya-network:v2"],
            "netmonitor/laya-network:v2"
        ));
        assert!(!is_available(&["kev:4b-q4_K_M"], "kev:0.8b-q8_0"));
        assert!(is_available(&["meu-modelo"], "meu-modelo"));
    }

    #[test]
    fn tempo_sugerido_segue_a_medicao_e_o_hardware() {
        let tempo = |name: &str| spec(name).unwrap().suggested_timeout_ms();
        assert_eq!(
            tempo("netmonitor/laya-network:v2"),
            4_000,
            "p95 de 3,6 s + 10%"
        );
        assert_eq!(
            tempo("tev1:4b-q4_K_M"),
            MAX_TIMEOUT_MS,
            "GPU sem medição em CPU"
        );
        for spec in KNOWN_MODELS {
            assert!(
                (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&spec.suggested_timeout_ms()),
                "{}",
                spec.name
            );
        }
    }

    #[test]
    fn modelo_grande_tem_prazo_maior_para_carregar() {
        assert!(load_timeout("d3:2b") >= load_timeout("netmonitor/laya-network:v2"));
        assert_eq!(load_timeout("desconhecido"), DEFAULT_LOAD_TIMEOUT);
    }

    #[test]
    fn teto_de_opcoes_vem_da_ficha() {
        assert_eq!(max_choice_options("laya-network"), Some(26));
        assert_eq!(max_choice_options("kev:4b-q4_K_M"), None);
        assert_eq!(max_choice_options("desconhecido"), None);
    }
}
