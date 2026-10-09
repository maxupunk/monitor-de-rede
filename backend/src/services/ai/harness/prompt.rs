//! System prompt da sessão: papel, método de trabalho, formato da resposta,
//! o que o usuário marcou com `@` e o contexto da tela de onde o chat foi aberto.

use chrono::Utc;
use sea_orm::{ConnectionTrait, EntityTrait};

use super::tools::{ToolPolicy, ToolRegistry};
use crate::{
    dtos::ai::{AiMention, AiMentionKind},
    models::{alert_events, alert_rules, devices, monitors},
    services::ai::{
        mentions,
        settings::{AiContainerActionMode, AiSettings},
    },
};

/// Onde o usuário estava quando abriu o chat e o que marcou na pergunta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatContext {
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
    pub alert_id: Option<i64>,
    pub rule_id: Option<i64>,
    /// Marcados com `@` na última pergunta — o alvo explícito dela.
    pub mentions: Vec<AiMention>,
}

// Cada caractere daqui vai em toda rodada de toda pergunta: regra nova entra
// curta, e só se mudar o comportamento da IA.
const METHOD: &str = "MÉTODO:
1. Cortesia: responda sem ferramentas. Ferramenta só quando a pergunta pedir dados.
2. Mínimo de consultas, e todas as já previstas na mesma rodada (cada rodada reenvia a conversa). \
Aparelho: get_device_detail; alertas: get_alerts; visão geral: get_system_summary.
3. Conclua só com dados. Vários alertas: causa raiz pela topologia primeiro. Origem de alerta: explain_alert; \
regra ruidosa: ajuste antes de excluir.
4. Logs, alertas, checagens e docker: grep — meça ('count'/'sources') antes de pedir 'lines'.
5. Gráficos já aparecem ao usuário: não os repita em texto.
6. O alvo são os marcados com @. Recurso ambíguo ou assunto novo: ask_user com opções antes de consultar.
7. Dúvida sobre o NetMonitor: search_system_docs ou get_alert_rules_guide.
Resultados de lista vêm como tabela: {columns, rows}.";

const ACTIVE_TOOLS: &str =
    "8. Testes ativos (ping, traceroute, portas, DNS, playbooks) dão o estado de agora; nunca invente o alvo.";

const ACTIONS: &str = "9. Ações (alerta, manutenção, monitor, regras) só rodam depois que o usuário confirma no chat. \
Proponha quando resolverem o pedido; ao receber 'awaiting_user_confirmation', diga em uma frase o que propôs e não repita a chamada.";

/// Regra das ações em container, conforme o modo configurado.
const fn container_rule(mode: AiContainerActionMode) -> Option<&'static str> {
    match mode {
        AiContainerActionMode::Off => None,
        AiContainerActionMode::Confirm => Some(
            "10. Containers (docker_container_action) só rodam depois da confirmação. Proponha com o container parado ou travado; ao receber 'awaiting_user_confirmation', não repita a chamada.",
        ),
        AiContainerActionMode::Auto => Some(
            "10. Containers (docker_container_action) rodam na hora: só quando pedido ou com o container parado ou travado — nunca por tentativa. Diga em uma frase o que fez.",
        ),
    }
}

/// Uma linha por grupo do catálogo: nome, para que serve e ferramentas.
///
/// Lista todos, carregados ou não: o system prompt não muda quando um grupo
/// entra, e o cache de prefixo do provedor continua valendo.
fn catalog_section(policy: ToolPolicy) -> Option<String> {
    let registry = ToolRegistry::new(policy);
    let lines: Vec<String> = registry
        .available_groups()
        .into_iter()
        .map(|group| {
            format!(
                "- {}: {} ({})",
                group.id(),
                group.purpose(),
                registry.tool_names(group).join(", ")
            )
        })
        .collect();
    (!lines.is_empty()).then(|| {
        format!(
            "
FERRAMENTAS SOB DEMANDA — load_tools carrega os grupos (todos numa chamada; ficam na conversa):
{}
",
            lines.join(
                "
"
            )
        )
    })
}

/// Prompt de uma troca de cortesia: papel e estilo, sem método nem contexto
/// — não há o que consultar.
#[must_use]
pub fn build_small_talk_prompt(settings: &AiSettings) -> String {
    format!(
        "Você é o NetMonitor AI, assistente de redes do NetMonitor. Responda à cortesia em uma frase e ofereça ajuda com a rede.\n{}\n",
        settings.response_style.directive()
    )
}

async fn device_section<C: ConnectionTrait>(db: &C, id: i64) -> Option<String> {
    let device = devices::Entity::find_by_id(id).one(db).await.ok()??;
    Some(format!(
        "- Dispositivo #{}: {} ({}), IP {}, fabricante {}, status {}, último contato {}",
        device.id,
        device.name,
        device.r#type,
        device.ip_address.as_deref().unwrap_or("N/A"),
        device.vendor.as_deref().unwrap_or("desconhecido"),
        device.status,
        device
            .last_seen_at
            .map_or_else(|| "N/A".to_string(), |at| at.to_rfc3339())
    ))
}

async fn monitor_section<C: ConnectionTrait>(db: &C, id: i64) -> Option<String> {
    let monitor = monitors::Entity::find_by_id(id).one(db).await.ok()??;
    Some(format!(
        "- Monitor #{}: {} (tipo {}, status {}, intervalo {}s{})",
        monitor.id,
        monitor.name,
        monitor.r#type,
        monitor.status,
        monitor.interval_seconds,
        monitor
            .device_id
            .map(|device| format!(", dispositivo #{device}"))
            .unwrap_or_default()
    ))
}

async fn alert_section<C: ConnectionTrait>(db: &C, id: i64) -> Option<String> {
    let alert = alert_events::Entity::find_by_id(id).one(db).await.ok()??;
    let rule_hint = alert.alert_rule_id.map_or_else(
        || "sem regra vinculada (alerta de sistema/baseline)".to_string(),
        |rule_id| format!("regra #{rule_id}"),
    );
    Some(format!(
        "- Alerta #{}: [{}] {} — status {}, {}, desde {}{}{}. Dica: use explain_alert alert_id={} para diagnosticar a regra, alvos e fatos que dispararam este alerta.",
        alert.id,
        alert.severity,
        alert.message.as_deref().unwrap_or("sem mensagem"),
        alert.status,
        rule_hint,
        alert.started_at.to_rfc3339(),
        alert
            .device_id
            .map(|device| format!(", dispositivo #{device}"))
            .unwrap_or_default(),
        alert
            .monitor_id
            .map(|monitor| format!(", monitor #{monitor}"))
            .unwrap_or_default(),
        alert.id
    ))
}

async fn rule_section<C: ConnectionTrait>(db: &C, id: i64) -> Option<String> {
    let rule = alert_rules::Entity::find_by_id(id).one(db).await.ok()??;
    let value_str = match &rule.condition["value"] {
        serde_json::Value::String(text) => format!("\"{text}\""),
        other => other.to_string(),
    };
    Some(format!(
        "- Regra de alerta #{}: '{}' [{}] — condição: {} {} {value_str}, ativa: {}. Dica: use list_alert_rules para listar regras ou toggle_alert_rule / delete_alert_rule se o usuário desejar alterá-la.",
        rule.id,
        rule.name,
        rule.severity,
        rule.condition["field"].as_str().unwrap_or("?"),
        rule.condition["operator"].as_str().unwrap_or("?"),
        rule.enabled
    ))
}

/// Uma linha por marcação. O que sumiu do banco é dito como tal, para a IA
/// não procurar um aparelho que não existe mais.
async fn mention_section<C: ConnectionTrait>(db: &C, mention: &AiMention) -> String {
    let missing = || format!("- '{}' (marcado, mas não existe mais)", mention.label);
    let id = mention.id.parse::<i64>().ok();
    match mention.kind {
        AiMentionKind::Device => match id {
            Some(id) => device_section(db, id).await.unwrap_or_else(missing),
            None => missing(),
        },
        AiMentionKind::Monitor => match id {
            Some(id) => monitor_section(db, id).await.unwrap_or_else(missing),
            None => missing(),
        },
        AiMentionKind::Container => format!(
            "- Container Docker '{label}' (id {id}): get_docker_containers container='{label}'; logs com grep source='docker' container='{label}'",
            label = mention.label,
            id = mention.id,
        ),
        AiMentionKind::Source => mentions::source(&mention.id).map_or_else(missing, |source| {
            format!("- Fonte {}: {}", source.label, source.hint)
        }),
    }
}

/// Linhas de contexto da tela; vazio quando o chat foi aberto sem contexto.
async fn context_sections<C: ConnectionTrait>(db: &C, context: &ChatContext) -> Vec<String> {
    let mut sections = Vec::new();
    if let Some(id) = context.alert_id {
        sections.extend(alert_section(db, id).await);
    }
    if let Some(id) = context.rule_id {
        sections.extend(rule_section(db, id).await);
    }
    if let Some(id) = context.monitor_id {
        sections.extend(monitor_section(db, id).await);
    }
    if let Some(id) = context.device_id {
        sections.extend(device_section(db, id).await);
    }
    sections
}

/// Monta o system prompt.
///
/// Só entra o que vale para a conversa inteira: ele abre toda chamada ao
/// provedor, e qualquer byte diferente invalida o cache de prefixo de tudo o
/// que vem depois (ferramentas e histórico). A data vai sem hora pelo mesmo
/// motivo; a hora exata, as marcações e a tela vão em [`build_turn_context`].
///
/// `with_catalog` é falso quando a sessão já recebe todas as ferramentas
/// (rotinas automáticas): não há o que carregar.
#[must_use]
pub fn build_system_prompt(
    settings: &AiSettings,
    policy: ToolPolicy,
    with_catalog: bool,
) -> String {
    let today = Utc::now().format("%Y-%m-%d");
    let mut prompt = format!(
        "Você é o NetMonitor AI, engenheiro de redes sênior do NetMonitor. Hoje: {today} (a hora vem no <contexto>).\n\n{METHOD}\n"
    );
    if policy.allow_active {
        prompt.push_str(ACTIVE_TOOLS);
        prompt.push('\n');
    }
    if policy.allow_actions {
        prompt.push_str(ACTIONS);
        prompt.push('\n');
    }
    if let Some(rule) = container_rule(policy.container_actions) {
        prompt.push_str(rule);
        prompt.push('\n');
    }
    if with_catalog {
        prompt.extend(catalog_section(policy));
    }
    prompt.push('\n');
    prompt.push_str(settings.response_style.directive());
    prompt.push('\n');

    if let Some(custom) = settings
        .custom_system_prompt
        .as_deref()
        .map(str::trim)
        .filter(|custom| !custom.is_empty())
    {
        prompt.push_str("\nInstruções adicionais do administrador:\n");
        prompt.push_str(custom);
        prompt.push('\n');
    }
    prompt
}

/// O que muda a cada pergunta: a hora, o que foi marcado com `@` e a tela de
/// onde o chat foi aberto. Vai junto da última pergunta, não no system
/// prompt — assim a conversa anterior continua em cache.
pub async fn build_turn_context<C: ConnectionTrait>(db: &C, context: &ChatContext) -> String {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let mut block = format!("<contexto>\nAgora: {now}\n");

    if !context.mentions.is_empty() {
        block.push_str(
            "MARCADOS COM @ NA PERGUNTA (o alvo dela; têm prioridade sobre o contexto da tela):\n",
        );
        for mention in &context.mentions {
            block.push_str(&mention_section(db, mention).await);
            block.push('\n');
        }
    }

    let sections = context_sections(db, context).await;
    if !sections.is_empty() {
        block
            .push_str("TELA DE ONDE O CHAT FOI ABERTO (vale enquanto a pergunta for sobre ela):\n");
        block.push_str(&sections.join("\n"));
        block.push('\n');
    }
    block.push_str("</contexto>\n\n");
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::harness::{compaction::estimate_tokens, tools::ToolGroups};

    /// O que toda pergunta de verdade paga em cada rodada, antes de qualquer
    /// dado: o system prompt e o núcleo de ferramentas. Medido em 2.662
    /// tokens (era 3.038). Subir o teto é decisão, não acidente: cada token a
    /// mais aqui se paga em toda rodada de toda pergunta.
    #[test]
    fn custo_fixo_por_rodada_cabe_no_orcamento() {
        const BUDGET: u64 = 2_800;
        let settings = AiSettings {
            allow_actions: true,
            ..AiSettings::default()
        };
        let policy = ToolPolicy::from_settings(&settings);
        let prompt = build_system_prompt(&settings, policy, true);
        let core = ToolRegistry::new(policy).definitions_for(&ToolGroups::new());
        let fixed =
            estimate_tokens(&prompt) + estimate_tokens(&serde_json::to_string(&core).unwrap());
        assert!(
            fixed <= BUDGET,
            "prompt + núcleo = ~{fixed} tokens, acima do orçamento de {BUDGET}"
        );
    }
}
