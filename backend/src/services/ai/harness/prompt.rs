//! System prompt da sessão: papel, método de trabalho, formato da resposta e
//! o contexto da tela de onde o chat foi aberto.

use chrono::Utc;
use sea_orm::{ConnectionTrait, EntityTrait};

use super::tools::ToolPolicy;
use crate::{
    models::{alert_events, devices, monitors},
    services::ai::settings::AiSettings,
};

/// Onde o usuário estava quando abriu o chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatContext {
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
    pub alert_id: Option<i64>,
}

const METHOD: &str = "COMO TRABALHAR:
1. Fundamente o diagnóstico em dados, nunca em suposição. Comece pelo que o sistema já registrou: \
get_system_summary, get_alerts, get_device_detail, get_device_interfaces, get_monitor_history e get_device_metrics.
2. Vários alertas ao mesmo tempo: use analyze_root_cause antes de tratar cada um — um switch ou gateway caído derruba o que está abaixo dele.
3. 'Está pior que o normal?': compare_with_baseline. 'Em que horário piora?': get_hourly_pattern. \
'O que aconteceu às 14h?': get_incident_timeline.
4. Logs (syslog dos equipamentos e da aplicação): comece por get_logs_overview, que agrupa as mensagens por padrão; \
use search_logs só para ler as linhas de um padrão ou termo específico.
5. Gráficos: quando o usuário pedir ou a evolução no tempo ajudar, use chart_monitor_latency, chart_interface_traffic, \
chart_device_metric ou get_hourly_pattern. O gráfico aparece para o usuário automaticamente — não o reproduza em texto nem em tabela.
6. Dúvidas sobre configurar ou usar o NetMonitor: search_system_docs.
7. Se um nome for ambíguo ou não existir, liste as opções (list_devices, list_monitors) em vez de adivinhar.";

const ACTIVE_TOOLS: &str = "8. Testes ativos (ping_host, traceroute, scan_ports, dns_lookup, run_playbook) confirmam o estado de agora.";

const ACTIONS: &str = "9. Ações (acknowledge_alert, silence_alert, create_maintenance_window, create_monitor) só são \
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

/// Linhas de contexto da tela; vazio quando o chat foi aberto sem contexto.
async fn context_sections<C: ConnectionTrait>(db: &C, context: ChatContext) -> Vec<String> {
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
    context: ChatContext,
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

    let sections = context_sections(db, context).await;
    if !sections.is_empty() {
        prompt.push_str(
            "\nCONTEXTO DA TELA DE ONDE O CHAT FOI ABERTO (use-o quando o usuário não citar outro alvo):\n",
        );
        prompt.push_str(&sections.join("\n"));
        prompt.push('\n');
    }
    prompt
}
