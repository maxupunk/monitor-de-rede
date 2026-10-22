//! Modelos de decisão para a tela escolher: todos os do registro oficial do
//! Ollaya que respondem às perguntas do NetMonitor, com a ficha de cada um, e
//! os que o Ollaya já tem instalados.
//!
//! **Compatível** quer dizer: responde a perguntas livres sobre texto dos dois
//! tipos que as decisões usam — `noul` (sim/não) e `choice` (escolha) — pela
//! `/api/decide`, com pelo menos as opções da maior escolha fixa (os tipos de
//! dispositivo). Ficam de fora:
//!
//! - `qwen3guard`: só responde às próprias perguntas de segurança (422 nas outras);
//! - `decider:2b-vision`: no máximo 10 opções por escolha;
//! - as tags `*-vision` de winnow e credence: o mesmo modelo de texto com um
//!   projetor de imagem, sem uso de texto documentado;
//! - as tags `*-fp16`/`*-fp32` do Laya: o mesmo modelo com a precisão fixada;
//! - `afm-de`/`afm-dd`: só existem num fork, fora do registro que a imagem
//!   `ghcr.io/ollaya-dev/ollaya` usa.
//!
//! A ficha ([`ModelSpec`]) guarda só o que o Ollaya publica (ollaya.dev,
//! páginas `/library/<família>` e `/results`); o resto — GPU recomendada,
//! tempo sugerido por pergunta, prazo de carregamento — sai de regras únicas
//! aqui, sem número solto por modelo. Nenhuma página cita português: "100+
//! línguas" é o que separa multilíngue de inglês.

use std::{cmp::Reverse, time::Duration};

use super::{
    benchmark::{Benchmark, USEFUL_PERMILLE},
    client::DeciderClient,
    config::AiDeciderSettings,
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

/// Onde o Ollaya mediu a latência publicada — e onde o modelo deve rodar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hardware {
    /// Medido num i9-13900K: roda bem sem placa de vídeo.
    Cpu,
    /// Medido numa RTX 4090: em CPU, segundos por pergunta.
    Gpu,
}

/// A ficha de um modelo conhecido, como o Ollaya a publica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: &'static str,
    /// Outros nomes do mesmo modelo no Ollaya (`kev` e `kev:latest` são o 4B).
    pub aliases: &'static [&'static str],
    pub family: &'static str,
    pub description: &'static str,
    pub language: DeciderModelLanguage,
    pub download_mb: Option<u32>,
    /// Memória com o modelo carregado, quando publicada.
    pub memory_mb: Option<u32>,
    /// Mediana publicada para cinco perguntas, no hardware de [`Self::hardware`].
    pub latency_ms: u32,
    pub hardware: Hardware,
    /// Acerto no typed-decisions, em milésimos — só quando comparável (sem
    /// o benchmark no treino).
    pub accuracy_permille: Option<u16>,
    /// Teto de opções por escolha, quando o modelo tem um.
    pub max_choice_options: Option<u16>,
    /// Resultado no teste público (`decider::benchmark`), quando avaliado.
    pub benchmark: Option<Benchmark>,
    /// Recomendado para o NetMonitor (um em CPU, um em GPU).
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
    memory_mb: None,
    latency_ms: 0,
    hardware: Hardware::Cpu,
    accuracy_permille: None,
    max_choice_options: None,
    benchmark: None,
    recommended: false,
    bundled: false,
};

/// Medido em CPU pelo NetMonitor: com um modelo Laya carregado, de 3 a 4 GB.
const DECIDER_MEMORY_MB: u32 = 4_000;

/// Faixa do "tempo máximo por pergunta" da configuração.
const MIN_TIMEOUT_MS: u32 = 1_500;
const MAX_TIMEOUT_MS: u32 = 10_000;

impl ModelSpec {
    #[must_use]
    pub const fn gpu_recommended(&self) -> bool {
        matches!(self.hardware, Hardware::Gpu)
    }

    /// Tempo máximo por pergunta sugerido. A latência publicada é de cinco
    /// perguntas; a escolha de ferramentas do chat faz por volta de dez. Em
    /// CPU a medição já é a do hardware comum (×3 de folga); em GPU ela é de
    /// uma RTX 4090, e uma placa comum é mais lenta (×6). Arredonda para
    /// cima, de 500 em 500 ms, dentro da faixa da configuração.
    #[must_use]
    pub const fn suggested_timeout_ms(&self) -> u32 {
        let factor = match self.hardware {
            Hardware::Cpu => 3,
            Hardware::Gpu => 6,
        };
        let raw = self.latency_ms * factor;
        let rounded = raw.div_ceil(500) * 500;
        if rounded < MIN_TIMEOUT_MS {
            MIN_TIMEOUT_MS
        } else if rounded > MAX_TIMEOUT_MS {
            MAX_TIMEOUT_MS
        } else {
            rounded
        }
    }

    /// Tirar o modelo do disco: 30 s por GB baixado, de 2 a 15 minutos — um
    /// Laya sai em 5 a 20 s, um 9B tem quase 20 GB.
    #[must_use]
    pub fn load_timeout(&self) -> Duration {
        let secs = self.download_mb.map_or(0, |mb| u64::from(mb) * 30 / 1_000);
        Duration::from_secs(secs.clamp(120, 900))
    }

    /// A nota que recomenda e ordena, em milésimos: a do benchmark das
    /// decisões do NetMonitor quando o modelo foi medido nele (as perguntas
    /// reais, em CPU); senão a do teste público.
    #[must_use]
    pub fn score_permille(&self) -> Option<u16> {
        decisions_benchmark::result(self.name)
            .map(decisions_benchmark::ModelResult::permille)
            .or(self.benchmark.map(|benchmark| benchmark.accuracy_permille))
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
            accuracy: self
                .accuracy_permille
                .map(|permille| f64::from(permille) / 1_000.0),
            max_choice_options: self.max_choice_options,
            suggested_timeout_ms: self.suggested_timeout_ms(),
            recommended: self.recommended,
            bundled: self.bundled,
        }
    }
}

/// Os modelos compatíveis, agrupados por família: primeiro os multilíngues
/// (Laya, que é o padrão, depois os de GPU), depois os em inglês que rodam em
/// CPU, por fim os em inglês que pedem GPU.
pub const KNOWN_MODELS: &[ModelSpec] = &[
    // ── Laya (Convai Innovations): encoders leves, o padrão ──
    ModelSpec {
        name: "laya",
        benchmark: Some(Benchmark::new(369, 381, 19)),
        family: "Laya",
        description: "Laya: roteador entre laya:multilingual e laya:en. O mais leve, mas perto do chute no teste público (37%)",
        language: Multilingual,
        download_mb: Some(1_536),
        memory_mb: Some(DECIDER_MEMORY_MB),
        latency_ms: 307,
        accuracy_permille: Some(361),
        max_choice_options: Some(125),
        ..BASE
    },
    ModelSpec {
        name: "laya:multilingual",
        benchmark: Some(Benchmark::new(369, 381, 19)),
        family: "Laya",
        description: "Laya: mmBERT 322M, 100+ línguas, mas perto do chute no teste público (37%)",
        language: Multilingual,
        download_mb: Some(683),
        memory_mb: Some(DECIDER_MEMORY_MB),
        latency_ms: 307,
        accuracy_permille: Some(362),
        max_choice_options: Some(250),
        ..BASE
    },
    ModelSpec {
        name: "laya:en",
        benchmark: Some(Benchmark::new(429, 417, 41)),
        family: "Laya",
        description: "Laya: ModernBERT-large 421M, só inglês",
        download_mb: Some(853),
        memory_mb: Some(DECIDER_MEMORY_MB),
        latency_ms: 619,
        accuracy_permille: Some(361),
        max_choice_options: Some(125),
        ..BASE
    },
    ModelSpec {
        name: "laya:typed-decisions",
        benchmark: Some(Benchmark::new(429, 357, 43)),
        family: "Laya",
        description: "Laya: ModernBERT-large ajustado nas decisões tipadas, só inglês",
        download_mb: Some(852),
        memory_mb: Some(DECIDER_MEMORY_MB),
        // Sem medição própria em CPU: o mesmo backbone do laya:en.
        latency_ms: 619,
        ..BASE
    },
    // ── Multilíngues para GPU: os mais precisos ──
    ModelSpec {
        name: "winnow:e4b",
        family: "Winnow",
        description: "Winnow (EldanRing): Gemma 4 E4B, 100+ línguas e entre os mais precisos (0,722) no teste do autor",
        language: Multilingual,
        download_mb: Some(8_000),
        memory_mb: Some(9_000),
        latency_ms: 89,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(722),
        max_choice_options: Some(64),
        ..BASE
    },
    ModelSpec {
        name: "winnow:12b",
        aliases: &["winnow"],
        family: "Winnow",
        description: "Winnow (EldanRing): Gemma 4 12B, 100+ línguas, sem calibração",
        language: Multilingual,
        download_mb: Some(12_700),
        memory_mb: Some(14_000),
        latency_ms: 131,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(702),
        max_choice_options: Some(64),
        ..BASE
    },
    ModelSpec {
        name: "credence:e4b",
        aliases: &["credence"],
        family: "Credence",
        description: "Credence (Txoka): Winnow-E4B refinado, 100+ línguas, o mais preciso (0,724)",
        language: Multilingual,
        download_mb: Some(8_700),
        latency_ms: 97,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(724),
        max_choice_options: Some(64),
        ..BASE
    },
    ModelSpec {
        name: "credence:e4b-calibrated",
        family: "Credence",
        description: "Credence (Txoka): a versão com a melhor calibração das probabilidades, 100+ línguas",
        language: Multilingual,
        download_mb: Some(8_700),
        // Sem medição própria: o mesmo motor do credence:e4b.
        latency_ms: 97,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(722),
        max_choice_options: Some(64),
        ..BASE
    },
    ModelSpec {
        name: "cygnet:12b",
        aliases: &["cygnet"],
        family: "Cygnet",
        description: "Cygnet (blockbrain-ai): Gemma 4 12B, 100+ línguas, até 20 opções por escolha",
        language: Multilingual,
        download_mb: Some(12_700),
        latency_ms: 202,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(683),
        max_choice_options: Some(20),
        ..BASE
    },
    ModelSpec {
        name: "arbiter:4b",
        aliases: &["arbiter"],
        family: "Arbiter",
        description: "Arbiter (Zyot Lab): Gemma 3 4B, 100+ línguas, até 16 opções por escolha",
        language: Multilingual,
        download_mb: Some(8_800),
        memory_mb: Some(8_000),
        latency_ms: 154,
        hardware: Hardware::Gpu,
        max_choice_options: Some(16),
        ..BASE
    },
    // ── NetMonitor: ajustado nas decisões dele, o padrão ──
    ModelSpec {
        name: "netmonitor/decima-network:v4f",
        aliases: &["decima-network", "netmonitor/decima-network"],
        family: "Decima Network",
        description: "Decima Network (NetMonitor): o decima:small ajustado nas decisões de rede do NetMonitor — 92% no benchmark dele, em português, e 0,3 s por decisão em CPU. Vem instalado com o NetMonitor",
        language: Multilingual,
        download_mb: Some(510),
        latency_ms: 306,
        recommended: true,
        bundled: true,
        ..BASE
    },
    // ── Multilíngues leves para CPU ──
    ModelSpec {
        name: "decima:base",
        aliases: &["decima"],
        family: "Decima",
        description: "Decima: mmBERT-base, multilíngue e leve em CPU, um pouco mais preciso que o Laya (0,495)",
        language: Multilingual,
        download_mb: Some(1_300),
        latency_ms: 438,
        accuracy_permille: Some(495),
        ..BASE
    },
    ModelSpec {
        name: "decima:agent",
        family: "Decima",
        description: "Decima: ajustado para decisões de agentes de código (comandos, segredos, ferramentas), multilíngue",
        language: Multilingual,
        download_mb: Some(1_300),
        latency_ms: 439,
        accuracy_permille: Some(486),
        ..BASE
    },
    ModelSpec {
        name: "decima:small",
        family: "Decima",
        description: "Decima: o mais rápido em CPU que o Ollaya roda (122M), avaliado em 20 línguas",
        language: Multilingual,
        download_mb: Some(510),
        latency_ms: 146,
        accuracy_permille: Some(432),
        ..BASE
    },
    ModelSpec {
        name: "largha:150m-fp32",
        aliases: &["largha"],
        family: "Largha",
        description: "Largha: mmBERT-small (144M), multilíngue e rápido em CPU, mas perto do chute nas decisões do NetMonitor",
        language: Multilingual,
        download_mb: Some(600),
        latency_ms: 174,
        ..BASE
    },
    // ── Em inglês, leves para CPU ──
    ModelSpec {
        name: "kev:0.8b",
        benchmark: Some(Benchmark::new(631, 619, 86)),
        family: "Kev",
        description: "Kev (Jared Palmer): Qwen3.5 0,8B — o melhor custo-benefício em CPU: o mais preciso abaixo de 1B no teste público, também em português",
        download_mb: Some(1_800),
        latency_ms: 1_270,
        accuracy_permille: Some(460),
        ..BASE
    },
    ModelSpec {
        name: "decision:eos",
        aliases: &["decision"],
        family: "Decision",
        description: "Decision (vLLM Semantic Router): Qwen3.5 0,8B, inglês e chinês",
        download_mb: Some(1_500),
        latency_ms: 2_000,
        ..BASE
    },
    ModelSpec {
        name: "nli:deberta-v3-large",
        aliases: &["nli"],
        family: "NLI",
        description: "NLI (Moritz Laurer): classificador zero-shot DeBERTa-v3, só inglês, sem calibração",
        download_mb: Some(884),
        latency_ms: 781,
        accuracy_permille: Some(548),
        ..BASE
    },
    ModelSpec {
        name: "nli:modernbert-large",
        family: "NLI",
        description: "NLI (Moritz Laurer): classificador zero-shot ModernBERT, só inglês, sem calibração",
        download_mb: Some(799),
        latency_ms: 594,
        accuracy_permille: Some(515),
        ..BASE
    },
    ModelSpec {
        name: "gliclass:large",
        aliases: &["gliclass"],
        family: "GLiClass",
        description: "GLiClass (Knowledgator): classificador zero-shot, só inglês, fraco em sim/não",
        download_mb: Some(1_800),
        latency_ms: 563,
        accuracy_permille: Some(477),
        ..BASE
    },
    ModelSpec {
        name: "von:1.1",
        aliases: &["von"],
        family: "Von",
        description: "Von: ModernBERT-large, só inglês; texto longo fica caro em CPU",
        download_mb: Some(1_600),
        latency_ms: 760,
        accuracy_permille: Some(447),
        ..BASE
    },
    // ── Em inglês, para GPU ──
    ModelSpec {
        name: "kev:4b",
        benchmark: Some(Benchmark::new(875, 869, 246)),
        aliases: &["kev"],
        family: "Kev",
        description: "Kev (Jared Palmer): Qwen3.5 4B — 87,5% no teste público, quase igual em português (87%)",
        download_mb: Some(9_500),
        memory_mb: Some(11_000),
        latency_ms: 354,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(669),
        ..BASE
    },
    ModelSpec {
        name: "kev:9b",
        benchmark: Some(Benchmark::new(881, 833, 561)),
        family: "Kev",
        description: "Kev (Jared Palmer): Qwen3.5 9B — 88% no teste público, mas cai para 83% em português; o 4B rende quase o mesmo com metade do tamanho",
        download_mb: Some(19_500),
        memory_mb: Some(18_000),
        latency_ms: 498,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(722),
        ..BASE
    },
    ModelSpec {
        name: "decider:2b",
        aliases: &["decider"],
        family: "Decider",
        description: "Decider (Mapika): Qwen3.5 2B, só inglês",
        download_mb: Some(3_800),
        memory_mb: Some(13_000),
        latency_ms: 190,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(591),
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "decider:4b",
        benchmark: Some(Benchmark::new(875, 857, 186)),
        family: "Decider",
        description: "Decider (Mapika): Qwen3.5 4B — 87,5% no teste público, 86% em português; o mais rápido dos 4B",
        download_mb: Some(8_400),
        memory_mb: Some(14_000),
        latency_ms: 520,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(680),
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "decider:0.8b",
        family: "Decider",
        description: "Decider (Mapika): Qwen3.5 0,8B, o menor Decider, só inglês",
        download_mb: Some(1_500),
        latency_ms: 155,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(506),
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "jeb:9b",
        aliases: &["jeb"],
        family: "Jeb",
        description: "Jeb (Jason Brashear): Qwen3.5 9B, só inglês (sem nota comparável: treinou no benchmark)",
        download_mb: Some(9_800),
        latency_ms: 124,
        hardware: Hardware::Gpu,
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "jeb:4b",
        benchmark: Some(Benchmark::new(905, 893, 205)),
        recommended: true,
        family: "Jeb",
        description: "Jeb (Jason Brashear): Qwen3.5 4B — o melhor 4B do teste público (90,5%, 89% em português); com GPU, o melhor custo-benefício",
        download_mb: Some(4_600),
        latency_ms: 96,
        hardware: Hardware::Gpu,
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "jeb:27b",
        family: "Jeb",
        description: "Jeb (Jason Brashear): Qwen3.8 27B para GPU de 24 GB, só inglês",
        download_mb: Some(16_800),
        latency_ms: 309,
        hardware: Hardware::Gpu,
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "jeeves:9b",
        aliases: &["jeeves"],
        family: "Jeeves",
        description: "Jeeves (PostHog): Qwen3.5 9B, bem calibrado, só inglês",
        download_mb: Some(17_900),
        memory_mb: Some(18_000),
        latency_ms: 838,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(680),
        ..BASE
    },
    ModelSpec {
        name: "clef:flash",
        benchmark: Some(Benchmark::new(875, 857, 618)),
        aliases: &["clef"],
        family: "Clef",
        description: "Clef-flash (Cloudflare): Qwen3.5 9B — 87,5% no teste público, 86% em português, bem calibrado; o mais lento dos aprovados",
        download_mb: Some(19_100),
        memory_mb: Some(19_000),
        latency_ms: 532,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(703),
        ..BASE
    },
    ModelSpec {
        name: "nimble:9b",
        aliases: &["nimble"],
        family: "Nimble",
        description: "Nimble (Bespoke Labs): Qwen3.5 9B, lento mesmo em GPU, só inglês",
        download_mb: Some(19_500),
        memory_mb: Some(18_000),
        latency_ms: 2_297,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(665),
        max_choice_options: Some(255),
        ..BASE
    },
    ModelSpec {
        name: "jevk5:4b",
        aliases: &["jevk5"],
        family: "JevK5",
        description: "JevK5 (alibiserikbay): Qwen3.5 4B, até 16 opções por escolha, só inglês",
        download_mb: Some(4_500),
        latency_ms: 105,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(625),
        max_choice_options: Some(16),
        ..BASE
    },
    ModelSpec {
        name: "snap:2b",
        aliases: &["snap"],
        family: "Snap",
        description: "Snap (logitlab): MiniCPM5 2B, inglês e italiano, até 26 opções, sem calibração",
        download_mb: Some(2_700),
        memory_mb: Some(3_000),
        latency_ms: 68,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(648),
        max_choice_options: Some(26),
        ..BASE
    },
    ModelSpec {
        name: "snap:2b-q4_k_m",
        family: "Snap",
        description: "Snap (logitlab): o mesmo 2B quantizado em 4 bits, menor, inglês e italiano",
        download_mb: Some(1_600),
        latency_ms: 63,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(660),
        max_choice_options: Some(26),
        ..BASE
    },
    ModelSpec {
        name: "clm:8b",
        aliases: &["clm"],
        family: "CLM",
        description: "CLM (Contrastive-LM): feito para agentes e jogos — perto do acaso em triagem geral, só inglês",
        download_mb: Some(16_500),
        memory_mb: Some(16_000),
        latency_ms: 500,
        hardware: Hardware::Gpu,
        accuracy_permille: Some(357),
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

/// Teto de opções por escolha do modelo; `None` quando ele não tem um.
#[must_use]
pub fn max_choice_options(model: &str) -> Option<usize> {
    spec(model)
        .and_then(|spec| spec.max_choice_options)
        .map(usize::from)
}

/// O modelo está na lista do Ollaya (instalados ou carregados), por qualquer
/// nome dele: `kev:4b` vale para quem baixou `kev`. O apelido é comparado
/// exato — `kev` não pode casar com um `kev:9b` instalado.
///
/// Nome sem tag (o roteador `laya`) mantém a regra de sempre: qualquer
/// `laya:*` instalado conta.
#[must_use]
pub fn is_available<S: AsRef<str>>(names: &[S], model: &str) -> bool {
    match spec(model) {
        Some(spec) if spec.name.contains(':') => {
            names.iter().any(|name| spec.answers_to(name.as_ref()))
        }
        Some(spec) => local_models::is_installed(names, spec.name),
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
            benchmark: spec.benchmark.map(|benchmark| benchmark.view()),
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
    fn padrao_e_o_decima_network_em_cpu_e_vem_instalado() {
        let padrao = spec(super::super::config::DEFAULT_DECIDER_MODEL).unwrap();
        assert_eq!(padrao.name, "netmonitor/decima-network:v4f");
        assert!(padrao.recommended && padrao.bundled);
        assert!(!padrao.gpu_recommended());
        assert!(
            padrao
                .score_permille()
                .is_some_and(|permille| permille >= 900),
            "nota do benchmark das decisões do NetMonitor"
        );
        assert!(
            padrao.suggested_timeout_ms() <= super::super::config::DEFAULT_DECIDER_TIMEOUT_MS,
            "o padrão cabe no tempo padrão da configuração"
        );
    }

    #[test]
    fn um_recomendado_em_cpu_e_um_em_gpu() {
        let recomendados: Vec<&ModelSpec> = KNOWN_MODELS
            .iter()
            .filter(|spec| spec.recommended)
            .collect();
        assert_eq!(recomendados.len(), 2);
        assert!(recomendados.iter().any(|spec| !spec.gpu_recommended()));
        assert!(recomendados.iter().any(|spec| spec.gpu_recommended()));
        assert!(
            recomendados.iter().all(|spec| spec
                .score_permille()
                .is_some_and(|permille| permille >= 850)),
            "recomendado tem nota alta (benchmark do NetMonitor ou teste público)"
        );
        assert!(
            recomendados.iter().all(|spec| spec.language == Multilingual
                || spec
                    .benchmark
                    .is_some_and(|benchmark| benchmark.portuguese_permille >= 600)),
            "recomendado atende português"
        );
    }

    #[test]
    fn lista_vai_da_maior_nota_a_menor_e_os_perto_do_chute_no_fim() {
        let mut ordem: Vec<&ModelSpec> = KNOWN_MODELS.iter().collect();
        ordem.sort_by_key(|spec| spec.rank());
        let nomes: Vec<&str> = ordem.iter().map(|spec| spec.name).collect();
        assert_eq!(
            &nomes[..2],
            ["netmonitor/decima-network:v4f", "jeb:4b"],
            "os recomendados abrem a lista"
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
        let primeiro_perto_do_chute = ordem
            .iter()
            .position(|spec| spec.score_permille().is_some_and(|p| p < USEFUL_PERMILLE))
            .expect("há modelos perto do chute");
        assert!(
            ordem[primeiro_perto_do_chute..]
                .iter()
                .all(|spec| spec.score_permille().is_some_and(|p| p < USEFUL_PERMILLE)),
            "os perto do chute vão para o fim: {nomes:?}"
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
    fn toda_ficha_tem_latencia_e_descricao() {
        for spec in KNOWN_MODELS {
            assert!(spec.latency_ms > 0, "{}", spec.name);
            assert!(!spec.description.is_empty(), "{}", spec.name);
            assert!(!spec.family.is_empty(), "{}", spec.name);
        }
    }

    #[test]
    fn apelido_resolve_para_o_tamanho_padrao_da_familia() {
        assert_eq!(spec("kev").map(|spec| spec.name), Some("kev:4b"));
        assert_eq!(spec("KEV:latest").map(|spec| spec.name), Some("kev:4b"));
        assert_eq!(spec("winnow").map(|spec| spec.name), Some("winnow:12b"));
        assert_eq!(spec("decima").map(|spec| spec.name), Some("decima:base"));
        assert!(spec("qwen3guard").is_none(), "incompatível fica fora");
        assert!(spec("modelo-proprio").is_none());
    }

    #[test]
    fn disponivel_pelo_apelido_sem_confundir_tamanhos() {
        assert!(
            is_available(&["kev:latest"], "kev:4b"),
            "baixou `kev`, tem o 4B"
        );
        assert!(is_available(&["kev:4b"], "kev:4b"));
        assert!(!is_available(&["kev:9b"], "kev:4b"), "o 9B não é o 4B");
        assert!(!is_available(&["kev:0.8b"], "kev:9b"));
        assert!(is_available(&["laya:latest"], "laya"));
        assert!(is_available(&["meu-modelo"], "meu-modelo"));
    }

    #[test]
    fn tempo_sugerido_segue_a_latencia_e_o_hardware() {
        let tempo = |name: &str| spec(name).unwrap().suggested_timeout_ms();
        assert_eq!(tempo("laya:en"), 2_000, "619 ms em CPU × 3");
        assert_eq!(tempo("kev:4b"), 2_500, "354 ms numa 4090 × 6");
        assert_eq!(tempo("nimble:9b"), MAX_TIMEOUT_MS, "o teto da configuração");
        assert_eq!(tempo("decima:small"), MIN_TIMEOUT_MS, "o piso");
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
        assert!(load_timeout("kev:9b") > load_timeout("kev:4b"));
        assert!(load_timeout("kev:4b") > load_timeout("laya"));
        assert_eq!(load_timeout("laya"), Duration::from_secs(120));
        assert_eq!(load_timeout("desconhecido"), DEFAULT_LOAD_TIMEOUT);
    }

    #[test]
    fn teto_de_opcoes_vem_da_ficha() {
        assert_eq!(max_choice_options("jevk5"), Some(16));
        assert_eq!(max_choice_options("kev:4b"), None);
        assert_eq!(max_choice_options("desconhecido"), None);
    }
}
