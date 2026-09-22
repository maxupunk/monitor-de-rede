//! System prompt da sessão: papel, método de trabalho, formato da resposta,
//! o que o usuário marcou com `@` e o contexto da tela de onde o chat foi aberto.

use chrono::Utc;
use sea_orm::{ConnectionTrait, EntityTrait};

use super::tools::ToolPolicy;
use crate::{
    dtos::ai::{AiMention, AiMentionKind},
    models::{alert_events, devices, monitors},
    services::ai::{mentions, settings::AiSettings},
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
1. Fundamente o diagnóstico em dados, nunca em suposição. Comece pelo que o sistema já registrou: \
get_system_summary, get_alerts, get_device_detail, get_device_interfaces, get_monitor_history e get_device_metrics.
2. Vários alertas ao mesmo tempo: use analyze_root_cause antes de tratar cada um — um switch ou gateway caído derruba o que está abaixo dele.
3. 'Está pior que o normal?': compare_with_baseline. 'Em que horário piora?': get_hourly_pattern. \
'O que aconteceu às 14h?': get_incident_timeline.
4. Logs, mensagens de alertas e falhas de checagem: para um panorama dos logs, get_logs_overview (agrupa por padrão). \
Para procurar algo específico, grep — meça antes de ler: output 'count' ou 'sources' diz quanto e onde; \
só então peça 'lines' (com device/hours estreitos) e context apenas se precisar ver o que veio antes/depois. \
Use regex para alternativas ('link (down|flap)') e exclude para tirar ruído.
5. Gráficos: quando o usuário pedir ou a evolução no tempo ajudar, use chart_monitor_latency, chart_interface_traffic, \
chart_device_metric ou get_hourly_pattern. O gráfico aparece para o usuário automaticamente — não o reproduza em texto nem em tabela.
6. Dúvidas sobre configurar ou usar o NetMonitor: search_system_docs.
7. Recursos marcados com @ são o alvo da pergunta: use-os direto. Se não estiver claro de qual dispositivo ou recurso vem a \
informação — a conversa era sobre um equipamento e agora a pergunta é de outro assunto (ex: era a borda, agora é a bateria ou o MPPT), \
ou o nome é ambíguo —, não assuma o assunto anterior nem o contexto da tela: chame ask_user com os candidatos \
(list_devices, list_monitors) como opções, antes de consultar qualquer outra coisa.
8. Containers Docker do servidor: get_docker_containers; logs de um container: grep source='docker'.";

const ACTIVE_TOOLS: &str = "9. Testes ativos (ping_host, traceroute, scan_ports, dns_lookup, run_playbook) confirmam o estado de agora.";

const ACTIONS: &str = "10. Ações (acknowledge_alert, silence_alert, create_maintenance_window, create_monitor) só são \
executadas depois que o usuário confirma no chat. Proponha a ação quando ela resolver o pedido; ao receber \
'awaiting_user_confirmation', diga em uma frase o que foi proposto e não repita a chamada.";

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
pub async fn build_system_prompt<C: ConnectionTrait>(
    db: &C,
    settings: &AiSettings,
    policy: ToolPolicy,
    context: &ChatContext,
) -> String {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let mut prompt = format!(
        "Você é o NetMonitor AI, engenheiro de redes sênior integrado ao NetMonitor. Agora: {now}.\n\n{METHOD}\n"
    );
    if policy.allow_active {
        prompt.push_str(ACTIVE_TOOLS);
        prompt.push('\n');
    }
    if policy.allow_actions {
        prompt.push_str(ACTIONS);
        prompt.push('\n');
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

    if !context.mentions.is_empty() {
        prompt.push_str(
            "\nMARCADOS COM @ NA PERGUNTA (o alvo dela; têm prioridade sobre o contexto da tela):\n",
        );
        for mention in &context.mentions {
            prompt.push_str(&mention_section(db, mention).await);
            prompt.push('\n');
        }
    }

    let sections = context_sections(db, context).await;
    if !sections.is_empty() {
        prompt.push_str(
            "\nCONTEXTO DA TELA DE ONDE O CHAT FOI ABERTO (vale enquanto a pergunta for sobre ele):\n",
        );
        prompt.push_str(&sections.join("\n"));
        prompt.push('\n');
    }
    prompt
}
