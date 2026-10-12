//! Onde o Laya pede as decisões: o Ollaya (nesta máquina ou em outra) ou um
//! serviço de modelos de decisão na nuvem.
//!
//! Por trás de todos há só três formatos de API ([`DecisionProtocol`], ver
//! `laya::protocols`): o nativo do Ollaya, o System One da TypeSafe (que Jev,
//! OpenRouter, Vercel, LiteLLM e o Clef da Cloudflare seguem) e o Decisions
//! da OpenAI. Cada provedor aqui é um preset — formato, endereço padrão,
//! modelos conhecidos e o que a tela precisa pedir (chave, ID da conta) —
//! para o operador escolher pelo nome, sem saber qual formato está por trás.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dtos::ai::{LayaProviderModel, LayaProviderView};

/// Quem responde às decisões.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LayaProvider {
    /// Ollaya nesta máquina (o serviço do compose) ou em outra.
    #[default]
    Ollaya,
    /// Jev, da TypeSafe.
    Typesafe,
    /// Jev pelo OpenRouter.
    Openrouter,
    /// Clef, nos Workers AI da Cloudflare.
    Cloudflare,
    /// OpenAI Decisions (`gpt-6-luna`).
    Openai,
    /// Qualquer endereço que fale um dos formatos (Vercel, LiteLLM, Kev…).
    Custom,
}

/// Formato da API.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DecisionProtocol {
    /// `POST /api/decide`, com download e carga de modelos.
    Ollaya,
    /// `POST /v1/systemone` — o formato da TypeSafe.
    #[default]
    Systemone,
    /// Workers AI: System One com o ID da conta na URL e a resposta em `result`.
    Cloudflare,
    /// `POST /v1/decisions` — o formato da OpenAI.
    Openai,
}

/// O preset de um provedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderPreset {
    pub provider: LayaProvider,
    pub label: &'static str,
    pub description: &'static str,
    /// `None` no "Outro": o operador escolhe.
    pub protocol: Option<DecisionProtocol>,
    pub default_base_url: &'static str,
    pub default_model: &'static str,
    /// Modelos que o provedor oferece, com o que cada um é.
    pub models: &'static [(&'static str, &'static str)],
    pub requires_key: bool,
    pub requires_account: bool,
    /// Onde o operador cria a chave.
    pub key_url: Option<&'static str>,
    /// Os textos saem da rede: a tela avisa antes de ligar.
    pub cloud: bool,
    /// Tempo máximo por pergunta sugerido (rede incluída). No Ollaya vale o
    /// do modelo (`laya::catalog`).
    pub suggested_timeout_ms: u32,
}

impl ProviderPreset {
    /// Baixa, lista e carrega modelos na memória — só o Ollaya.
    #[must_use]
    pub fn manages_models(&self) -> bool {
        self.protocol == Some(DecisionProtocol::Ollaya)
    }

    #[must_use]
    pub fn view(&self) -> LayaProviderView {
        LayaProviderView {
            id: self.provider,
            label: self.label.to_string(),
            description: self.description.to_string(),
            protocol: self.protocol,
            default_base_url: self.default_base_url.to_string(),
            default_model: self.default_model.to_string(),
            models: self
                .models
                .iter()
                .map(|(name, description)| LayaProviderModel {
                    name: (*name).to_string(),
                    description: (*description).to_string(),
                })
                .collect(),
            requires_key: self.requires_key,
            requires_account: self.requires_account,
            key_url: self.key_url.map(str::to_string),
            cloud: self.cloud,
            manages_models: self.manages_models(),
            suggested_timeout_ms: self.suggested_timeout_ms,
        }
    }
}

/// Em nuvem a rede entra na conta: folga para uma ida e volta à internet.
const CLOUD_TIMEOUT_MS: u32 = 3_000;

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        provider: LayaProvider::Ollaya,
        label: "Ollaya",
        description: "Modelos abertos rodando nesta máquina (o serviço do compose) ou em outra da sua rede. Nada sai da rede.",
        protocol: Some(DecisionProtocol::Ollaya),
        default_base_url: super::config::DEFAULT_LAYA_BASE_URL,
        default_model: super::config::DEFAULT_LAYA_MODEL,
        models: &[],
        requires_key: false,
        requires_account: false,
        key_url: None,
        cloud: false,
        suggested_timeout_ms: super::config::DEFAULT_LAYA_TIMEOUT_MS,
    },
    ProviderPreset {
        provider: LayaProvider::Typesafe,
        label: "TypeSafe (Jev)",
        description: "Jev, o modelo de decisão da TypeSafe, direto na API dela. Cobrado por token de entrada.",
        protocol: Some(DecisionProtocol::Systemone),
        default_base_url: "https://api.typesafe.ai",
        default_model: "jev-latest",
        models: &[
            ("jev-latest", "Jev estável (hoje o jev-1.13.0)"),
            ("jev-preview", "Jev em prévia: as novidades antes do estável"),
        ],
        requires_key: true,
        requires_account: false,
        key_url: Some("https://console.typesafe.ai"),
        cloud: true,
        suggested_timeout_ms: CLOUD_TIMEOUT_MS,
    },
    ProviderPreset {
        provider: LayaProvider::Openrouter,
        label: "OpenRouter (Jev)",
        description: "O Jev da TypeSafe pela conta do OpenRouter, sem lista de espera. Cobrado por token de entrada.",
        protocol: Some(DecisionProtocol::Systemone),
        default_base_url: "https://openrouter.ai/api",
        default_model: "~typesafe/jev-latest",
        models: &[
            ("~typesafe/jev-latest", "Jev estável, sempre a versão mais nova"),
            ("typesafe/jev-1.13", "Jev 1.13, versão fixa"),
        ],
        requires_key: true,
        requires_account: false,
        key_url: Some("https://openrouter.ai/settings/keys"),
        cloud: true,
        suggested_timeout_ms: 4_000,
    },
    ProviderPreset {
        provider: LayaProvider::Cloudflare,
        label: "Cloudflare (Clef)",
        description: "Clef, o modelo de decisão aberto da Cloudflare, nos Workers AI. Pede o ID da conta e um token com permissão de Workers AI.",
        protocol: Some(DecisionProtocol::Cloudflare),
        default_base_url: "https://api.cloudflare.com/client/v4",
        default_model: "clef-flash",
        models: &[
            ("clef-flash", "Clef-flash 9B: dezenas de milissegundos, o mais barato"),
            ("clef", "Clef 27B: o mais preciso da Cloudflare"),
        ],
        requires_key: true,
        requires_account: true,
        key_url: Some("https://dash.cloudflare.com/profile/api-tokens"),
        cloud: true,
        suggested_timeout_ms: CLOUD_TIMEOUT_MS,
    },
    ProviderPreset {
        provider: LayaProvider::Openai,
        label: "OpenAI Decisions",
        description: "A API Decisions da OpenAI (beta), com o gpt-6-luna. Cobrado por token de entrada.",
        protocol: Some(DecisionProtocol::Openai),
        default_base_url: "https://api.openai.com",
        default_model: "gpt-6-luna",
        models: &[("gpt-6-luna", "O modelo de decisão da OpenAI")],
        requires_key: true,
        requires_account: false,
        key_url: Some("https://platform.openai.com/api-keys"),
        cloud: true,
        suggested_timeout_ms: CLOUD_TIMEOUT_MS,
    },
    ProviderPreset {
        provider: LayaProvider::Custom,
        label: "Outro compatível",
        description: "Qualquer endereço no formato System One (TypeSafe) ou Decisions (OpenAI): Vercel AI Gateway, LiteLLM, um Kev servido por você…",
        protocol: None,
        default_base_url: "",
        default_model: "",
        models: &[],
        requires_key: false,
        requires_account: false,
        key_url: None,
        cloud: true,
        suggested_timeout_ms: CLOUD_TIMEOUT_MS,
    },
];

/// O preset do provedor. Todo provedor tem um.
#[must_use]
pub fn preset(provider: LayaProvider) -> &'static ProviderPreset {
    PRESETS
        .iter()
        .find(|preset| preset.provider == provider)
        .unwrap_or(&PRESETS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_provedor_tem_preset() {
        for provider in [
            LayaProvider::Ollaya,
            LayaProvider::Typesafe,
            LayaProvider::Openrouter,
            LayaProvider::Cloudflare,
            LayaProvider::Openai,
            LayaProvider::Custom,
        ] {
            assert_eq!(preset(provider).provider, provider);
        }
    }

    #[test]
    fn so_o_ollaya_baixa_modelos_e_so_ele_fica_na_rede() {
        for preset in PRESETS {
            assert_eq!(
                preset.manages_models(),
                preset.provider == LayaProvider::Ollaya,
                "{}",
                preset.label
            );
            assert_eq!(preset.cloud, preset.provider != LayaProvider::Ollaya);
        }
    }

    #[test]
    fn nuvem_conhecida_pede_chave_e_traz_o_modelo_padrao_na_lista() {
        for preset in PRESETS
            .iter()
            .filter(|preset| preset.cloud && preset.provider != LayaProvider::Custom)
        {
            assert!(preset.requires_key, "{}", preset.label);
            assert!(preset.key_url.is_some(), "{}", preset.label);
            assert!(
                preset
                    .models
                    .iter()
                    .any(|(name, _)| *name == preset.default_model),
                "{}",
                preset.label
            );
        }
    }
}
