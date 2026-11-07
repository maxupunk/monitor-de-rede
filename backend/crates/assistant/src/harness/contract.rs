//! O chat como um modelo o vê — prompt de sistema, contexto da pergunta e
//! ferramentas —, para o Laya-tuning medir modelos fora daqui (o benchmark de
//! chat offline).
//!
//! `backend/contracts/ai_chat.json` é gerado pelo teste deste módulo, como os
//! bindings do `ts-rs`: o `cargo test` o reescreve quando uma ferramenta, um
//! schema ou o prompt muda, e o `sync-check` do Laya-tuning acusa a diferença.
//! A data e a hora viram marcadores (`{{today}}`, `{{now}}`): o arquivo só muda
//! quando o contrato muda.
//!
//! As peças vão separadas para o benchmark remontar as duas formas que o
//! agente usa ([`super::agent`]): com o Decisor, o núcleo, os grupos que ele
//! escolheu e o `load_tools` com o catálogo no prompt; sem ele, tudo de saída.
//!
//! A política é a do chat mais aberto (testes ativos, ações e containers com
//! confirmação): todo grupo entra, e cada cenário do benchmark escolhe o que
//! libera.

use chrono::Utc;
use sea_orm::ConnectionTrait;
use serde::Serialize;

use super::{
    agent::MAX_ITERATIONS,
    prompt::{build_system_prompt, build_turn_context, ChatContext},
    tools::{ToolGroup, ToolGroups, ToolPolicy, ToolRegistry, LOAD_TOOLS},
};
use crate::{
    drivers::traits::AiTool,
    ollama,
    settings::{AiAutomationMode, AiSettings, DEFAULT_OLLAMA_NUM_CTX},
};

/// O chat com todo grupo liberado, as ações e os testes ativos sob confirmação.
const POLICY: ToolPolicy = ToolPolicy {
    allow_active: true,
    actions: AiAutomationMode::Confirm,
    confirm_active: true,
    interactive: true,
    container_actions: AiAutomationMode::Confirm,
    areas: netmonitor_core::areas::AreaSet::ALL,
    writable: netmonitor_core::areas::AreaSet::ALL,
};

const TODAY: &str = "{{today}}";
const NOW: &str = "{{now}}";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatContract {
    /// Prompt quando sobra grupo de fora: leva o catálogo do `load_tools`.
    pub system_prompt_on_demand: String,
    /// Prompt com todo grupo já carregado (sem Decisor).
    pub system_prompt_everything: String,
    /// Bloco que abre a última pergunta do usuário (sem tela nem `@`).
    pub turn_context: String,
    /// Rodadas (chamada ao modelo + ferramentas) por pergunta.
    pub max_rounds: usize,
    /// Janela pedida ao Ollama (`options.num_ctx`), a padrão da tela.
    pub num_ctx: u64,
    /// Teto da resposta (`max_tokens`) no estilo padrão; `None` = sem teto.
    pub max_output_tokens: Option<u32>,
    /// `reasoning_effort` que vai ao Ollama na configuração padrão.
    pub reasoning_effort: Option<&'static str>,
    /// Sempre enviadas, nesta ordem.
    pub core: Vec<AiTool>,
    /// Vai depois do núcleo enquanto sobrar grupo a carregar.
    pub load_tools: Option<AiTool>,
    /// Grupos sob demanda, na ordem do catálogo; carregados entram no fim da
    /// lista, na ordem em que foram pedidos.
    pub groups: Vec<ContractGroup>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractGroup {
    pub id: &'static str,
    pub purpose: &'static str,
    /// A afirmação que o Decisor avalia para pré-carregar o grupo.
    pub intent: &'static str,
    pub tools: Vec<AiTool>,
}

/// O contrato atual do chat.
pub async fn chat_contract<C: ConnectionTrait>(db: &C) -> ChatContract {
    let settings = AiSettings::default();
    let registry = ToolRegistry::new(POLICY);
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let prompt =
        |with_catalog| build_system_prompt(&settings, POLICY, with_catalog).replace(&today, TODAY);
    let (load_tools, core): (Vec<AiTool>, Vec<AiTool>) = registry
        .definitions_for(&ToolGroups::new())
        .into_iter()
        .partition(|tool| tool.function.name == LOAD_TOOLS);
    let groups = registry
        .available_groups()
        .into_iter()
        .map(|group: ToolGroup| ContractGroup {
            id: group.id(),
            purpose: group.purpose(),
            intent: group.intent(),
            tools: registry.group_definitions(group),
        })
        .collect();
    ChatContract {
        system_prompt_on_demand: prompt(true),
        system_prompt_everything: prompt(false),
        turn_context: placeholder_now(&build_turn_context(db, &ChatContext::default()).await),
        max_rounds: MAX_ITERATIONS,
        num_ctx: DEFAULT_OLLAMA_NUM_CTX,
        max_output_tokens: settings.response_style.max_output_tokens(),
        reasoning_effort: ollama::reasoning_effort(&settings),
        core,
        load_tools: load_tools.into_iter().next(),
        groups,
    }
}

/// `Agora: 2026-10-29 12:00:00 UTC` → `Agora: {{now}}`.
fn placeholder_now(block: &str) -> String {
    block
        .lines()
        .map(|line| {
            if line.starts_with("Agora: ") {
                format!("Agora: {NOW}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if block.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hora_vira_marcador() {
        assert_eq!(
            placeholder_now("<contexto>\nAgora: 2026-10-29 12:00:00 UTC\n</contexto>\n\n"),
            "<contexto>\nAgora: {{now}}\n</contexto>\n\n"
        );
    }
}
