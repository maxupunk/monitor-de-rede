//! System prompt da sessão: papel, método de trabalho, formato da resposta,
//! o que o usuário marcou com `@` e o contexto da tela de onde o chat foi aberto.

use chrono::Utc;
use sea_orm::{ConnectionTrait, EntityTrait};

use super::tools::{ToolPolicy, ToolRegistry};
use crate::{
    dtos::ai::{AiMention, AiMentionKind},
    models::{alert_events, devices, monitors},
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
    /// Marcados com `@` na última pergunta — o alvo explícito dela.
    pub mentions: Vec<AiMention>,
}

const METHOD: &str = "COMO TRABALHAR:
1. Use ferramentas só quando a pergunta precisar de dados do sistema. Saudação, agradecimento ou conversa: responda direto, sem consultar nada.
2. Consulte o mínimo que responde, e peça na mesma rodada tudo o que já sabe que vai precisar — cada rodada reenvia a conversa inteira. \
Um aparelho: get_device_detail. Alertas: get_alerts. Visão geral da rede: get_system_summary.
3. Fundamente o diagnóstico nos dados, nunca em suposição. Vários alertas juntos: a causa raiz pela topologia vem antes de tratar um a um. \
De onde vem um alerta (regra, alvo, fatos que casaram): explain_alert.
4. Logs, mensagens de alertas e falhas de checagem: grep — meça antes de ler ('count' ou 'sources'), depois 'lines' com device/hours \
estreitos; regex para alternativas ('link (down|flap)'), exclude para tirar ruído.
5. Gráficos aparecem para o usuário automaticamente — não os reproduza em texto nem em tabela.
6. Marcados com @ são o alvo da pergunta: use-os direto. Se não estiver claro de qual dispositivo ou recurso vem a informação \
(a conversa era sobre um equipamento e a pergunta mudou de assunto — ex: era a borda, agora é a bateria ou o MPPT —, ou o nome é ambíguo), \
não assuma o assunto anterior nem o contexto da tela: chame ask_user com os candidatos como opções, antes de consultar outra coisa.
7. Dúvidas sobre configurar ou usar o NetMonitor: search_system_docs.";

const ACTIVE_TOOLS: &str =
    "8. Testes ativos (ping, traceroute, portas, DNS, playbooks) confirmam o estado de agora.";

const ACTIONS: &str = "9. Ações (reconhecer/silenciar alerta, janela de manutenção, criar monitor, criar/excluir regra de alerta) só são \
executadas depois que o usuário confirma no chat. Proponha a ação quando ela resolver o pedido; ao receber \
'awaiting_user_confirmation', diga em uma frase o que foi proposto e não repita a chamada.";

/// Regra das ações em container, conforme o modo configurado.
const fn container_rule(mode: AiContainerActionMode) -> Option<&'static str> {
    match mode {
        AiContainerActionMode::Off => None,
        AiContainerActionMode::Confirm => Some(
            "10. Containers (docker_container_action: iniciar, parar, reiniciar) só rodam depois que o usuário confirma. Proponha quando o diagnóstico mostrar o container parado ou travado; ao receber 'awaiting_user_confirmation', não repita a chamada.",
        ),
        AiContainerActionMode::Auto => Some(
            "10. Containers (docker_container_action: iniciar, parar, reiniciar) rodam na hora, sem confirmação. Só aja quando o usuário pediu ou o diagnóstico mostrou o container parado ou travado — nunca por tentativa. Diga em uma frase o que fez.",
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
FERRAMENTAS SOB DEMANDA — carregue com load_tools (todos os grupos necessários numa chamada) antes de usar; um grupo carregado continua disponível na conversa:
{}
",
            lines.join("
")
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
    Some(format!(
        "- Alerta #{}: [{}] {} — status {}, desde {}{}{}",
        alert.id,
        alert.severity,
        alert.message.as_deref().unwrap_or("sem mensagem"),
        alert.status,
        alert.started_at.to_rfc3339(),
        alert
            .device_id
            .map(|device| format!(", dispositivo #{device}"))
            .unwrap_or_default(),
        alert
            .monitor_id
            .map(|monitor| format!(", monitor #{monitor}"))
            .unwrap_or_default()
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
        "Você é o NetMonitor AI, engenheiro de redes sênior integrado ao NetMonitor. Hoje: {today} (a hora exata vem no bloco <contexto> da pergunta).\n\n{METHOD}\n"
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
