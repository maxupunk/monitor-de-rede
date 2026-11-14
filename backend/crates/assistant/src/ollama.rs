//! Integração com a API nativa do Ollama para gerenciamento e instalação de modelos.
//!
//! Fornece consulta de modelos instalados (`/api/tags`), catálogo de modelos
//! recomendados para o NetMonitor e streaming em tempo real do progresso de
//! download (`/api/pull`). Enquanto a URL da tela apontar para esta máquina,
//! o Ollama é procurado onde o compose o deixa ([`OLLAMA`], ver
//! [`super::local_server`]).

use std::time::Duration;

use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    context_window::sources::parse_ollama_show,
    local_models::{self, BundledOllamaModel, PullTarget},
    local_server::LocalServer,
    settings::AiSettings,
};
use crate::dtos::{
    ModelPullProgress, OllamaModelItem, OllamaModelsResponse, OllamaRecommendedModel,
};
use netmonitor_core::shared::errors::{AppError, AppResult};

/// O Ollama do compose: o serviço `ollama`, e `OLLAMA_URL` definida pela
/// instalação (o override de host a define).
pub static OLLAMA: LocalServer = LocalServer::new("Ollama", "ollama", 11434, "OLLAMA_URL");

const CHAT_MODELS_RELEASE_URL: &str =
    "https://github.com/maxupunk/monitor-de-rede/releases/download/chat-models";

fn lfm3_default_parameters() -> serde_json::Value {
    serde_json::json!({
        "temperature": 0,
        "top_k": 1,
        "repeat_penalty": 1.05,
        "num_ctx": 262144,
    })
}

/// Modelos de chat empacotados pelo NetMonitor: quando o usuário pede o pull
/// pela tela (ou em um Ollama sem o volume pré-populado), os pesos GGUF vêm
/// direto da release do GitHub do NetMonitor (`CHAT_MODELS_URL`), disponíveis
/// em 3 quantizações (`Q8_0` / `Q8_K`, `Q5_K_M` e `Q4_K_M`).
pub static BUNDLED_MODELS: &[BundledOllamaModel] = &[
    BundledOllamaModel {
        canonical_name: "lfm3-netmonitor:latest",
        aliases: &[
            "lfm3-netmonitor:q8_0",
            "lfm3-netmonitor:q8_k",
            "netmonitor/lfm3-netmonitor:latest",
            "netmonitor/lfm3-netmonitor:q8_0",
            "netmonitor/lfm3-netmonitor:q8_k",
        ],
        release_url_env: "CHAT_MODELS_URL",
        default_release_url: CHAT_MODELS_RELEASE_URL,
        gguf_filename: "lfm3-netmonitor-q8_0.gguf",
        gguf_digest: "sha256:5c1fa051a965afff8f349811cfa5f6c3ff3817ab7c252e63785af02f0a3d73b4",
        gguf_size: 1_846_237_728,
        template: "{{ .Prompt }}",
        renderer: "lfm3",
        parser: "lfm3",
        create_parameters: lfm3_default_parameters,
    },
    BundledOllamaModel {
        canonical_name: "lfm3-netmonitor:q5_k_m",
        aliases: &["netmonitor/lfm3-netmonitor:q5_k_m"],
        release_url_env: "CHAT_MODELS_URL",
        default_release_url: CHAT_MODELS_RELEASE_URL,
        gguf_filename: "lfm3-netmonitor-q5_k_m.gguf",
        gguf_digest: "sha256:99a0a2fb1ad3823bb9df4574a9744917c3e84cd880ef86a96e267dab393e6bae",
        gguf_size: 1_258_662_432,
        template: "{{ .Prompt }}",
        renderer: "lfm3",
        parser: "lfm3",
        create_parameters: lfm3_default_parameters,
    },
    BundledOllamaModel {
        canonical_name: "lfm3-netmonitor:q4_k_m",
        aliases: &["netmonitor/lfm3-netmonitor:q4_k_m"],
        release_url_env: "CHAT_MODELS_URL",
        default_release_url: CHAT_MODELS_RELEASE_URL,
        gguf_filename: "lfm3-netmonitor-q4_k_m.gguf",
        gguf_digest: "sha256:6de95b0c82ce1ebf84f86db4f366898a7591001356bbf42996d2923c389f5228",
        gguf_size: 1_099_627_040,
        template: "{{ .Prompt }}",
        renderer: "lfm3",
        parser: "lfm3",
        create_parameters: lfm3_default_parameters,
    },
];

/// Localiza a receita empacotada para um modelo do NetMonitor, se houver.
#[must_use]
pub fn bundled_model(model: &str) -> Option<&'static BundledOllamaModel> {
    BUNDLED_MODELS.iter().find(|spec| spec.matches(model))
}

/// Um modelo do catálogo recomendado: a tabela que a tela mostra e de onde
/// sai a janela de contexto que o campo `num_ctx` assume ao escolher o modelo.
pub struct RecommendedModel {
    pub name: &'static str,
    pub description: &'static str,
    pub parameter_size: &'static str,
    /// Janela nativa do modelo, em tokens (a `*.context_length` do GGUF).
    pub context_length: u64,
    pub tool_calling_optimized: bool,
    /// Ajustado com as ferramentas do NetMonitor: conhece o catálogo e o
    /// `load_tools`, então o chat dispensa o roteamento do Decisor e manda o
    /// prompt enxuto (o Decisor continua revisando as ações).
    pub netmonitor_trained: bool,
}

/// Modelos recomendados para execução no Ollama com o NetMonitor.
pub static RECOMMENDED_MODELS: &[RecommendedModel] = &[
    RecommendedModel {
        name: "lfm3-netmonitor:latest",
        description: "LFM3 NetMonitor 1.7B v6 (Q8_0 / Q8_K — Máxima precisão, padrão) — Especialista nas ferramentas do NetMonitor com jargão técnico NOC/ISP brasileiro (redes, Wi-Fi IEEE 802.11, fibra óptica GPON/SFP+, roteadores, Docker, backups, dashboard e web_search), treinado para responder só com o que as consultas mostraram: 1,85 GB, nenhum recurso inventado em 128 cenários reais e 100% em segurança (nota 99,0 blind / 97,6 bench)",
        parameter_size: "1.7B · Q8_0",
        context_length: 262_144,
        tool_calling_optimized: true,
        netmonitor_trained: true,
    },
    RecommendedModel {
        name: "lfm3-netmonitor:q5_k_m",
        description: "LFM3 NetMonitor 1.7B v6 (Q5_K_M — Equilibrado) — Mesma especialização NOC/ISP e nas ferramentas do NetMonitor com 32% menos memória: 1,26 GB, nenhum recurso inventado em 128 cenários reais e 100% em segurança (nota 98,2 blind / 97,6 bench)",
        parameter_size: "1.7B · Q5_K_M",
        context_length: 262_144,
        tool_calling_optimized: true,
        netmonitor_trained: true,
    },
    RecommendedModel {
        name: "lfm3-netmonitor:q4_k_m",
        description: "LFM3 NetMonitor 1.7B v6 (Q4_K_M — Ultraleve / Rápido) — Versão compacta de 4 bits para servidores com pouca RAM/CPU: 1,10 GB, 1 recurso inventado em 128 cenários reais e 100% em segurança no blind (nota 97,5 blind / 96,9 bench)",
        parameter_size: "1.7B · Q4_K_M",
        context_length: 262_144,
        tool_calling_optimized: true,
        netmonitor_trained: true,
    },
    RecommendedModel {
        name: "ornith-1.5:9b",
        description: "Ornith 1.5 9B — Modelo recomendado de grande porte: autoaperfeiçoamento, raciocínio avançado, ferramentas de rede e janela nativa de 256k",
        parameter_size: "9B",
        context_length: 262_144,
        tool_calling_optimized: true,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "lfm3:latest",
        description: "LFM3 1.7B — Modelo base tri-híbrido para agentes locais: 1,1 GB, roda em CPU e chama ferramentas (76,8% no benchmark de chat antes do ajuste)",
        parameter_size: "1.7B",
        context_length: 262_144,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "llama3-groq-tool-use:8b",
        description: "Llama 3 Groq Tool Use 8B — Especialista em execução de ferramentas de rede e diagnósticos (Function Calling)",
        parameter_size: "8B",
        context_length: 8_192,
        tool_calling_optimized: true,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "llama3.2:3b",
        description: "Llama 3.2 3B — Modelo ultraleve e veloz da Meta, excelente relação entre consumo e acurácia",
        parameter_size: "3B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "llama3.2:1b",
        description: "Llama 3.2 1B — Modelo mais compacto da Meta, consome menos de 2GB de RAM",
        parameter_size: "1B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "llama3.3:70b",
        description: "Llama 3.3 70B — Estado da arte da Meta em raciocínio, diagnóstico e código",
        parameter_size: "70B",
        context_length: 131_072,
        tool_calling_optimized: true,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "deepseek-r1:8b",
        description: "DeepSeek R1 8B — Especialista em raciocínio lógico avançado e arquitetura de redes",
        parameter_size: "8B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "deepseek-r1:14b",
        description: "DeepSeek R1 14B — Raciocínio profundo com alta capacidade analítica para sistemas",
        parameter_size: "14B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "qwen2.5-coder:7b",
        description: "Qwen 2.5 Coder 7B — Especialista em automação técnica, scripts e infraestrutura",
        parameter_size: "7B",
        context_length: 32_768,
        tool_calling_optimized: true,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "gemma4:e2b",
        description: "Gemma 4 e2b — Modelo ultraleve do Google, ideal para respostas instantâneas em servidores com recursos modestos",
        parameter_size: "2B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "gemma4:e4b",
        description: "Gemma 4 e4b — Equilíbrio ideal entre consumo de memória e capacidade de resolução de problemas",
        parameter_size: "4B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "qwen3:8b",
        description: "Qwen 3 8B — Excelente raciocínio lógico, análise estruturada e alta precisão em língua portuguesa",
        parameter_size: "8B",
        context_length: 40_960,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "qwen3.8:27b",
        description: "Qwen 3.8 27B — Alta precisão, raciocínio técnico avançado, suporte a ferramentas e janela de 256k",
        parameter_size: "27B",
        context_length: 262_144,
        tool_calling_optimized: true,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "granite4.2:latest",
        description: "Granite 4.2 — Modelo corporativo da IBM otimizado para operações de TI, automação e infraestrutura",
        parameter_size: "8B",
        context_length: 131_072,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "mixtral:8x7b",
        description: "Mixtral 8x7B — Arquitetura Mixture-of-Experts para análises complexas e correlações avançadas de rede",
        parameter_size: "46.7B",
        context_length: 32_768,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
    RecommendedModel {
        name: "phi4:14b",
        description: "Phi-4 14B — Modelo da Microsoft com excepcional raciocínio sintético e síntese técnica",
        parameter_size: "14B",
        context_length: 16_384,
        tool_calling_optimized: false,
        netmonitor_trained: false,
    },
];

/// Família de um modelo do Ollama: sem a tag e sem o namespace
/// (`netmonitor/lfm3-netmonitor:q4_k_m` ➔ `lfm3-netmonitor`).
fn model_family(model: &str) -> String {
    let name = model.trim().to_ascii_lowercase();
    let base = name.rsplit('/').next().unwrap_or(&name);
    base.split(':').next().unwrap_or(base).to_string()
}

/// `true` quando o modelo é de uma família treinada com as ferramentas do
/// NetMonitor no catálogo ([`RecommendedModel::netmonitor_trained`]): vale
/// para qualquer quantização ou tag da família (`lfm3-netmonitor:q5_k_m`,
/// `lfm3-netmonitor:minha-tag`, `netmonitor/lfm3-netmonitor:latest`).
#[must_use]
pub fn is_known_netmonitor_trained(model: &str) -> bool {
    let family = model_family(model);
    !family.is_empty()
        && RECOMMENDED_MODELS
            .iter()
            .any(|known| known.netmonitor_trained && model_family(known.name) == family)
}

/// Normaliza a URL base do Ollama, removendo sufixos `/v1` e barras finais.
pub fn normalize_ollama_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "http://localhost:11434".to_string();
    }
    let mut url = trimmed.trim_end_matches('/').to_string();
    if url.ends_with("/v1") {
        url = url[..url.len() - 3].trim_end_matches('/').to_string();
    }
    if url.is_empty() {
        "http://localhost:11434".to_string()
    } else {
        url
    }
}

/// Teto mínimo de `max_tokens` com o raciocínio ligado: o que o modelo pensa
/// conta no teto, e os 1.024 do estilo Direto acabavam antes da resposta
/// (benchmark de chat do Laya-tuning, `lfm3`: com 4.096, nenhuma resposta
/// cortada e a nota de 75 para 83).
pub const REASONING_MIN_OUTPUT_TOKENS: u32 = 4096;

/// O piso de `max_tokens` do chat no Ollama: só com o raciocínio ligado.
#[must_use]
pub const fn min_output_tokens(settings: &AiSettings) -> Option<u32> {
    if settings.ollama_reasoning {
        Some(REASONING_MIN_OUTPUT_TOKENS)
    } else {
        None
    }
}

/// O `reasoning_effort` que o chat envia ao Ollama: `none` com o raciocínio
/// desligado (o padrão), nada com ele ligado — aí vale o do modelo.
#[must_use]
pub const fn reasoning_effort(settings: &AiSettings) -> Option<&'static str> {
    if settings.ollama_reasoning {
        None
    } else {
        Some("none")
    }
}

/// A raiz (sem `/v1`) onde o Ollama de fato está. A URL de outra máquina é
/// soberana; a desta máquina, quando ninguém responde nela, é trocada pelo
/// primeiro endereço do compose que responder. Ninguém respondendo, fica a
/// da tela — e o erro da chamada diz onde se tentou.
pub async fn locate_root(configured: &str) -> String {
    let root = normalize_ollama_base_url(configured);
    if !OLLAMA.means_this_machine(&root) {
        return root;
    }
    OLLAMA.locate(&root).await.unwrap_or(root)
}

/// A URL do protocolo OpenAI (`.../v1`) para o chat: a da tela, ou a do
/// endereço onde o Ollama foi achado ([`locate_root`]).
pub async fn locate_openai_base(configured: &str) -> String {
    let root = locate_root(configured).await;
    if root == normalize_ollama_base_url(configured) {
        configured.to_owned()
    } else {
        format!("{root}/v1")
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagDetails {
    parameter_size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagItem {
    name: String,
    size: Option<u64>,
    modified_at: Option<String>,
    details: Option<OllamaTagDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaTagItem>>,
}

/// Rótulo curto de uma janela de contexto: `262_144` ➔ `256k`, `40_960` ➔ `40k`.
#[must_use]
pub fn context_window_label(tokens: u64) -> String {
    if tokens < 1024 {
        tokens.to_string()
    } else {
        format!("{}k", tokens / 1024)
    }
}

/// A janela nativa de cada modelo instalado, pelo `/api/show` (em paralelo:
/// o `/api/tags` não a informa). Quem não responder fica sem — a tela usa a
/// do catálogo ou a recomendada.
async fn fill_context_lengths(
    client: &reqwest::Client,
    root_url: &str,
    items: &mut [OllamaModelItem],
) {
    let show_url = format!("{root_url}/api/show");
    let lengths = futures::future::join_all(items.iter().map(|item| {
        let request = client
            .post(&show_url)
            .json(&serde_json::json!({ "model": item.name }));
        async move {
            let response = request.send().await.ok()?.error_for_status().ok()?;
            let show = response.json::<serde_json::Value>().await.ok()?;
            parse_ollama_show(&show).0
        }
    }))
    .await;
    for (item, length) in items.iter_mut().zip(lengths) {
        item.context_length = length;
    }
}

/// Consulta os modelos instalados no Ollama e retorna cruzados com os recomendados.
pub async fn list_models(base_url: &str) -> AppResult<OllamaModelsResponse> {
    let root_url = locate_root(base_url).await;
    let tags_url = format!("{root_url}/api/tags");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro no cliente HTTP: {e}")))?;

    let response = client.get(&tags_url).send().await;

    let (installed_items, is_online, error_message) = match response {
        Ok(res) if res.status().is_success() => {
            if let Ok(tags) = res.json::<OllamaTagsResponse>().await {
                let mut items: Vec<OllamaModelItem> = tags
                    .models
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| OllamaModelItem {
                        netmonitor_trained: is_known_netmonitor_trained(&m.name),
                        name: m.name,
                        size: m.size,
                        parameter_size: m.details.and_then(|d| d.parameter_size),
                        modified_at: m.modified_at,
                        context_length: None,
                    })
                    .collect();
                fill_context_lengths(&client, &root_url, &mut items).await;
                items.sort_by_key(|m| {
                    BUNDLED_MODELS
                        .iter()
                        .position(|spec| spec.matches(&m.name))
                        .unwrap_or(usize::MAX)
                });
                (items, true, None)
            } else {
                (Vec::new(), true, None)
            }
        }
        Ok(res) => {
            let status = res.status();
            (
                Vec::new(),
                false,
                Some(format!("Ollama retornou status HTTP {status}")),
            )
        }
        Err(e) => (
            Vec::new(),
            false,
            Some(format!(
                "Não foi possível conectar ao Ollama em '{root_url}': {e}"
            )),
        ),
    };

    let installed_names: Vec<&str> = installed_items.iter().map(|m| m.name.as_str()).collect();

    let recommended: Vec<OllamaRecommendedModel> = RECOMMENDED_MODELS
        .iter()
        .map(|model| {
            let is_installed = local_models::is_installed(&installed_names, model.name)
                || bundled_model(model.name).is_some_and(|spec| {
                    spec.aliases
                        .iter()
                        .any(|alias| local_models::is_installed(&installed_names, alias))
                });

            OllamaRecommendedModel {
                name: model.name.to_string(),
                description: model.description.to_string(),
                parameter_size: model.parameter_size.to_string(),
                context_window: context_window_label(model.context_length),
                context_length: model.context_length,
                is_installed,
                tool_calling_optimized: model.tool_calling_optimized,
                netmonitor_trained: model.netmonitor_trained,
            }
        })
        .collect();

    Ok(OllamaModelsResponse {
        installed: installed_items,
        recommended,
        is_online,
        error_message,
    })
}

/// Inicia o download (pull) de um modelo no Ollama e transmite o progresso via stream assíncrono.
/// Para modelos ajustados do NetMonitor (`BUNDLED_MODELS`), baixa os pesos da release do
/// repositório do NetMonitor (`CHAT_MODELS_URL`) e registra no Ollama via `/api/blobs` + `/api/create`.
pub async fn pull_model_stream(
    base_url: &str,
    model: &str,
) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let model_name = model.trim();
    if model_name.is_empty() {
        return Err(AppError::validation("Nome do modelo não pode ser vazio"));
    }
    let root = locate_root(base_url).await;
    if let Some(spec) = bundled_model(model_name) {
        return local_models::stream_bundled_ollama_install(root, model_name.to_string(), spec)
            .await;
    }
    local_models::stream_pull(PullTarget {
        url: format!("{root}/api/pull"),
        body: serde_json::json!({ "name": model_name, "stream": true }),
        bearer: None,
        service: "Ollama",
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testa_normalizacao_de_url_do_ollama() {
        assert_eq!(normalize_ollama_base_url(""), "http://localhost:11434");
        assert_eq!(
            normalize_ollama_base_url("http://localhost:11434/v1"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_base_url("http://localhost:11434/v1/"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_base_url("http://192.168.1.50:11434/"),
            "http://192.168.1.50:11434"
        );
        assert_eq!(
            normalize_ollama_base_url("https://ollama.internal.net"),
            "https://ollama.internal.net"
        );
    }

    #[test]
    fn a_url_padrao_e_a_do_servico_do_compose() {
        assert_eq!(
            normalize_ollama_base_url(crate::settings::DEFAULT_OLLAMA_BASE_URL),
            OLLAMA.default_url()
        );
    }

    #[test]
    fn raciocinio_desligado_por_padrao_vai_como_none() {
        let mut settings = AiSettings::default();
        assert_eq!(reasoning_effort(&settings), Some("none"));
        assert_eq!(min_output_tokens(&settings), None);
        settings.ollama_reasoning = true;
        assert_eq!(reasoning_effort(&settings), None);
        assert_eq!(
            min_output_tokens(&settings),
            Some(REASONING_MIN_OUTPUT_TOKENS)
        );
    }

    #[tokio::test]
    async fn a_url_de_outra_maquina_nao_e_trocada() {
        assert_eq!(
            locate_openai_base("http://10.0.0.5:11434/v1").await,
            "http://10.0.0.5:11434/v1"
        );
    }

    #[test]
    fn familia_lfm3_netmonitor_e_treinada_com_o_netmonitor() {
        for model in [
            "lfm3-netmonitor:latest",
            "lfm3-netmonitor:q5_k_m",
            "lfm3-netmonitor:q4_k_m",
            "LFM3-NetMonitor",
            "netmonitor/lfm3-netmonitor:q8_0",
            "lfm3-netmonitor:minha-tag",
        ] {
            assert!(is_known_netmonitor_trained(model), "{model}");
        }
        for model in ["qwen2.5:7b", "lfm3:latest", "llama3.2:3b", "", "  "] {
            assert!(!is_known_netmonitor_trained(model), "{model:?}");
        }
    }

    #[test]
    fn rotulo_da_janela_em_k_binario() {
        assert_eq!(context_window_label(262_144), "256k");
        assert_eq!(context_window_label(131_072), "128k");
        assert_eq!(context_window_label(40_960), "40k");
        assert_eq!(context_window_label(512), "512");
    }

    #[test]
    fn todo_recomendado_tem_janela_que_a_tela_aceita() {
        for model in RECOMMENDED_MODELS {
            assert!(
                model.context_length >= 4096,
                "{} sem janela de contexto plausível",
                model.name
            );
        }
    }

    #[test]
    fn resolve_tres_quantizacoes_do_lfm3_netmonitor() {
        let q8 = bundled_model("lfm3-netmonitor:latest").expect("Q8_0 padrão");
        assert_eq!(q8.gguf_filename, "lfm3-netmonitor-q8_0.gguf");
        assert_eq!(
            bundled_model("lfm3-netmonitor:q8_0").map(|s| s.gguf_filename),
            Some("lfm3-netmonitor-q8_0.gguf")
        );
        assert_eq!(
            bundled_model("lfm3-netmonitor:q8_k").map(|s| s.gguf_filename),
            Some("lfm3-netmonitor-q8_0.gguf")
        );

        let q5 = bundled_model("lfm3-netmonitor:q5_k_m").expect("Q5_K_M");
        assert_eq!(q5.gguf_filename, "lfm3-netmonitor-q5_k_m.gguf");
        assert_eq!(q5.gguf_size, 1_258_662_432);

        let q4 = bundled_model("lfm3-netmonitor:q4_k_m").expect("Q4_K_M");
        assert_eq!(q4.gguf_filename, "lfm3-netmonitor-q4_k_m.gguf");
        assert_eq!(q4.gguf_size, 1_099_627_040);
    }
}
