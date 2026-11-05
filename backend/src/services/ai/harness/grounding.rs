//! Alvo de teste ativo precisa ter origem.
//!
//! Sem dado nenhum, um modelo pequeno "propõe" um ping em `10.0.0.1`,
//! `10.0.0.2` e `10.0.0.3` — endereços que ele não tem como conhecer — e o
//! usuário recebe três pedidos de confirmação inúteis (ou três testes contra
//! nada, sem a confirmação). Endereço interno só é testado se veio da
//! conversa (o usuário escreveu, uma consulta devolveu, o administrador pôs no
//! prompt) ou está cadastrado no inventário. Alvo público (`8.8.8.8`,
//! `google.com`) é conhecimento geral e segue livre, como o loopback.

use std::net::IpAddr;

use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use crate::{
    models::{_entities::alert_events, alert_rules, devices, monitors, scheduled_tasks},
    services::{
        ai::drivers::traits::AiMessage, alerts::contracts::AlertStatus, shared::errors::AppResult,
    },
};

/// Sufixos de nome que só resolvem dentro da rede do usuário.
pub const LOCAL_SUFFIXES: &[&str] = &[
    ".local",
    ".lan",
    ".home",
    ".internal",
    ".intranet",
    ".localdomain",
    ".corp",
    ".home.arpa",
];

/// Minúsculas, sem espaços, arroba de menção nem colchetes de IPv6.
fn normalize(target: &str) -> String {
    target
        .trim()
        .trim_start_matches('@')
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_lowercase()
}

/// Endereço que só existe na rede do usuário: o modelo não tem como sabê-lo
/// sozinho. Loopback não entra — testar a própria central é inofensivo.
#[must_use]
pub fn is_internal(target: &str) -> bool {
    let target = normalize(target);
    match target.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let [first, second, ..] = ip.octets();
            ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                // 100.64.0.0/10: CGNAT, o endereço que o provedor dá ao cliente.
                || (first == 100 && (64..128).contains(&second))
        }
        Ok(IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            ip.is_unspecified() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
        Err(_) => {
            target != "localhost"
                && (!target.contains('.')
                    || LOCAL_SUFFIXES.iter().any(|suffix| target.ends_with(suffix)))
        }
    }
}

/// Caractere que continua um IP ou hostname.
fn continues_host(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '-'
}

/// O texto cita o alvo como palavra inteira: `10.0.0.1` não casa dentro de
/// `10.0.0.12`, mas casa em `"ip":"10.0.0.1"`, `10.0.0.1:161` e no fim de uma
/// frase (`... o 10.0.0.1.`).
#[must_use]
pub fn mentions(text: &str, target: &str) -> bool {
    let target = normalize(target);
    if target.is_empty() {
        return false;
    }
    let text = text.to_lowercase();
    text.match_indices(&target).any(|(start, found)| {
        let before = text[..start].chars().next_back();
        let mut after = text[start + found.len()..].chars();
        let starts_clean = before.is_none_or(|c| !continues_host(c));
        let ends_clean = match after.next() {
            None => true,
            // Ponto ou hífen de pontuação: o que vem depois não é mais nome.
            Some('.' | '-') => after.next().is_none_or(|c| !c.is_alphanumeric()),
            Some(c) => !continues_host(c),
        };
        starts_clean && ends_clean
    })
}

/// O alvo apareceu no que a IA leu nesta sessão: pedido do usuário (com o
/// `<contexto>`), resultado de consulta ou o system prompt. O que a própria IA
/// escreveu não conta — é exatamente o que se quer conferir.
fn grounded_in_conversation(conversation: &[AiMessage], target: &str) -> bool {
    conversation
        .iter()
        .filter(|message| message.role != "assistant")
        .filter_map(|message| message.content.as_deref())
        .any(|content| mentions(content, target))
}

/// O alvo é o IP de um dispositivo ou o alvo de um monitor cadastrado (o
/// resultado da consulta pode ter sido podado da conversa).
async fn registered<C: ConnectionTrait>(db: &C, target: &str) -> AppResult<bool> {
    let wanted = normalize(target);
    let same = |value: &str| {
        let value = normalize(value);
        value == wanted || mentions(&value, &wanted)
    };
    let devices = devices::Entity::find().all(db).await?;
    if devices
        .iter()
        .any(|device| same(&device.name) || device.ip_address.as_deref().is_some_and(same))
    {
        return Ok(true);
    }
    let monitors = monitors::Entity::find().all(db).await?;
    Ok(monitors.iter().any(|monitor| {
        same(&monitor.name)
            || super::tools::monitor_target(&monitor.configuration).is_some_and(same)
    }))
}

/// Mensagem devolvida à IA quando o alvo foi inventado; `None` quando ele tem
/// origem. Falha do banco não bloqueia: o teste segue o caminho normal.
pub async fn invented_target<C: ConnectionTrait>(
    db: &C,
    conversation: &[AiMessage],
    target: &str,
) -> Option<String> {
    if !is_internal(target) || grounded_in_conversation(conversation, target) {
        return None;
    }
    if registered(db, target).await.unwrap_or(true) {
        return None;
    }
    Some(format!(
        "O alvo '{}' é um endereço interno que não veio do usuário nem de uma consulta: não invente alvos. Use os IPs de list_devices ou list_monitors, ou pergunte ao usuário com ask_user.",
        target.trim()
    ))
}

/// Tipo de recurso monitorado sobre o qual `ask_user` pede escolha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Container,
    Device,
    Monitor,
    Alert,
    AlertRule,
    ScheduledTask,
}

const GENERIC_QUESTION_TARGETS: &[&str] = &[
    "qual ação",
    "qual acao",
    "qual período",
    "qual periodo",
    "qual intervalo",
    "qual horário",
    "qual horario",
    "qual métrica",
    "qual metrica",
    "qual comando",
    "qual operação",
    "qual operacao",
    "qual tipo",
    "qual severidade",
    "o que deseja",
    "o que você deseja",
    "o que voce deseja",
    "como deseja",
    "por quanto tempo",
];

const NATURAL_HYPHENATED: &[&str] = &[
    "segunda-feira",
    "terça-feira",
    "terca-feira",
    "quarta-feira",
    "quinta-feira",
    "sexta-feira",
    "fim-de-semana",
    "passo-a-passo",
    "e-mail",
];

/// Identifica quando a pergunta de `ask_user` pede para escolher **qual**
/// recurso do inventário/Docker usar (e não qual ação/período).
#[must_use]
pub fn asking_which_resource(question: &str) -> Option<ResourceKind> {
    let q = question.trim().to_lowercase();
    let explicit_pairs: &[(&[&str], ResourceKind)] = &[
        (
            &[
                "qual container",
                "quais containers",
                "quais container",
                "qual contêiner",
                "quais contêineres",
                "qual conteiner",
                "quais conteineres",
                "qual o container",
                "qual o contêiner",
                "qual o conteiner",
            ],
            ResourceKind::Container,
        ),
        (
            &["qual regra", "quais regras", "qual a regra"],
            ResourceKind::AlertRule,
        ),
        (
            &["qual alerta", "quais alertas", "qual o alerta"],
            ResourceKind::Alert,
        ),
        (
            &[
                "qual tarefa",
                "quais tarefas",
                "qual agendamento",
                "qual a tarefa",
            ],
            ResourceKind::ScheduledTask,
        ),
        (
            &["qual monitor", "quais monitores", "qual o monitor"],
            ResourceKind::Monitor,
        ),
        (
            &[
                "qual dispositivo",
                "quais dispositivos",
                "qual equipamento",
                "quais equipamentos",
                "qual aparelho",
                "quais aparelhos",
                "qual roteador",
                "qual switch",
                "qual servidor",
                "qual host",
                "qual olt",
                "qual mikrotik",
            ],
            ResourceKind::Device,
        ),
    ];
    for (phrases, kind) in explicit_pairs {
        if phrases.iter().any(|phrase| q.contains(phrase)) {
            return Some(*kind);
        }
    }
    if GENERIC_QUESTION_TARGETS
        .iter()
        .any(|phrase| q.contains(phrase))
    {
        return None;
    }
    let asks_which = q.contains("qual ")
        || q.contains("quais ")
        || q.contains("escolha ")
        || q.contains("selecione ");
    if !asks_which {
        return None;
    }
    if q.contains("container") || q.contains("contêiner") || q.contains("conteiner") {
        Some(ResourceKind::Container)
    } else if q.contains("regra") {
        Some(ResourceKind::AlertRule)
    } else if q.contains("alerta") {
        Some(ResourceKind::Alert)
    } else if q.contains("tarefa") || q.contains("agendamento") {
        Some(ResourceKind::ScheduledTask)
    } else if q.contains("monitor") {
        Some(ResourceKind::Monitor)
    } else if [
        "dispositivo",
        "equipamento",
        "aparelho",
        "roteador",
        "switch",
        "servidor",
    ]
    .iter()
    .any(|word| q.contains(word))
    {
        Some(ResourceKind::Device)
    } else {
        None
    }
}

/// Detecta se uma opção de `ask_user` tem formato de identificador técnico
/// (ex.: `zabbix-proxy-01`, `redis-fila-arnoldo`, `#410 Queda de link`,
/// `10.0.0.50`, `ether1`) em vez de frase de escolha natural.
#[must_use]
pub fn looks_like_resource_identifier(option: &str) -> bool {
    let trimmed = option.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(rest) = trimmed.strip_prefix('#') {
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    let lower = trimmed.to_lowercase();
    for raw_token in lower.split_whitespace() {
        let token = raw_token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if token.is_empty() || NATURAL_HYPHENATED.contains(&token) {
            continue;
        }
        let has_slug_sep = (token.contains('-') || token.contains('_'))
            && token.chars().any(char::is_alphanumeric);
        if has_slug_sep {
            return true;
        }
        if token.chars().any(|c| c.is_ascii_digit()) && token.chars().any(char::is_alphabetic) {
            return true;
        }
        if token.contains('.') && is_internal(token) {
            return true;
        }
    }
    false
}

fn push_unique(list: &mut Vec<String>, candidate: impl Into<String>) {
    let candidate = candidate.into();
    let trimmed = candidate.trim();
    if !trimmed.is_empty() && !list.iter().any(|seen| seen.eq_ignore_ascii_case(trimmed)) {
        list.push(trimmed.to_string());
    }
}

/// Extrai valores de uma coluna (`col_name`) de resultados de ferramentas na
/// conversa, tanto no formato bruto (`[{"name": "x"}]`) quanto no formato
/// enxuto (`{"columns": ["name", ...], "rows": [["x", ...]]}`).
fn extract_tool_column(conversation: &[AiMessage], key: &str, col_name: &str) -> Vec<String> {
    let mut values = Vec::new();
    for content in conversation
        .iter()
        .filter(|message| message.role == "tool")
        .filter_map(|message| message.content.as_deref())
    {
        let Ok(parsed) = serde_json::from_str::<Value>(content) else {
            continue;
        };
        let Some(section) = parsed.get(key) else {
            continue;
        };
        if let Some(items) = section.as_array() {
            for item in items {
                if let Some(val) = item.get(col_name).and_then(Value::as_str) {
                    push_unique(&mut values, val);
                }
            }
        } else if let (Some(cols), Some(rows)) = (
            section.get("columns").and_then(Value::as_array),
            section.get("rows").and_then(Value::as_array),
        ) {
            if let Some(idx) = cols.iter().position(|c| c.as_str() == Some(col_name)) {
                for row in rows {
                    if let Some(val) = row
                        .as_array()
                        .and_then(|arr| arr.get(idx))
                        .and_then(Value::as_str)
                    {
                        push_unique(&mut values, val);
                    }
                }
            }
        }
    }
    values
}

fn containers_from_conversation(conversation: &[AiMessage]) -> Vec<String> {
    let mut list = extract_tool_column(conversation, "containers", "name");
    for extra in extract_tool_column(conversation, "usage", "name") {
        push_unique(&mut list, extra);
    }
    for content in conversation
        .iter()
        .filter(|message| message.role == "user")
        .filter_map(|message| message.content.as_deref())
    {
        for line in content.lines() {
            if let Some(rest) = line.trim().strip_prefix("- Container Docker '") {
                if let Some((name, _)) = rest.split_once('\'') {
                    push_unique(&mut list, name);
                }
            }
        }
    }
    for content in conversation
        .iter()
        .filter(|message| message.role == "assistant")
        .filter_map(|message| message.content.as_deref())
    {
        let mut rest = content;
        while let Some(start) = rest.find('`') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('`') else {
                break;
            };
            let candidate = after[..end].trim();
            if looks_like_resource_identifier(candidate) && !candidate.contains(' ') {
                push_unique(&mut list, candidate);
            }
            rest = &after[end + 1..];
        }
    }
    list
}

async fn candidates_for_kind(
    ctx: &AppContext,
    conversation: &[AiMessage],
    kind: ResourceKind,
) -> Vec<String> {
    let mut out = Vec::new();
    match kind {
        ResourceKind::Container => {
            for name in containers_from_conversation(conversation) {
                push_unique(&mut out, name);
            }
            for name in super::tools::list_all_container_names(ctx).await {
                push_unique(&mut out, name);
            }
        }
        ResourceKind::Device => {
            for name in extract_tool_column(conversation, "devices", "name") {
                push_unique(&mut out, name);
            }
            if let Ok(rows) = devices::Entity::find()
                .order_by_asc(devices::Column::Name)
                .all(&ctx.db)
                .await
            {
                for row in rows {
                    push_unique(&mut out, row.name);
                }
            }
        }
        ResourceKind::Monitor => {
            for name in extract_tool_column(conversation, "monitors", "name") {
                push_unique(&mut out, name);
            }
            if let Ok(rows) = monitors::Entity::find()
                .order_by_asc(monitors::Column::Name)
                .all(&ctx.db)
                .await
            {
                for row in rows {
                    push_unique(&mut out, row.name);
                }
            }
        }
        ResourceKind::Alert => {
            if let Ok(rows) = alert_events::Entity::find()
                .filter(alert_events::Column::Status.is_in(AlertStatus::OPEN))
                .order_by_desc(alert_events::Column::Id)
                .all(&ctx.db)
                .await
            {
                for row in rows {
                    let label = match row.message.as_deref() {
                        Some(msg) if !msg.trim().is_empty() => format!("#{} {}", row.id, msg.trim()),
                        _ => format!("#{}", row.id),
                    };
                    push_unique(&mut out, label);
                }
            }
        }
        ResourceKind::AlertRule => {
            for name in extract_tool_column(conversation, "rules", "name") {
                push_unique(&mut out, name);
            }
            if let Ok(rows) = alert_rules::Entity::find()
                .order_by_asc(alert_rules::Column::Name)
                .all(&ctx.db)
                .await
            {
                for row in rows {
                    push_unique(&mut out, row.name);
                }
            }
        }
        ResourceKind::ScheduledTask => {
            for name in extract_tool_column(conversation, "tasks", "name") {
                push_unique(&mut out, name);
            }
            if let Ok(rows) = scheduled_tasks::Entity::find()
                .order_by_asc(scheduled_tasks::Column::Name)
                .all(&ctx.db)
                .await
            {
                for row in rows {
                    push_unique(&mut out, row.name);
                }
            }
        }
    }
    out
}

fn option_matches_candidate(option: &str, candidates: &[String]) -> bool {
    let opt = normalize(option);
    if opt.is_empty() {
        return false;
    }
    candidates.iter().any(|cand| {
        let c = normalize(cand);
        c == opt || mentions(&c, &opt) || mentions(&opt, &c)
    })
}

/// Filtra opções inventadas de `ask_user` (ex.: containers/dispositivos do
/// dataset de treino que não existem na sessão) e, quando a pergunta pede para
/// escolher um recurso (container, dispositivo, alerta, regra), completa com
/// todos os recursos reais disponíveis no ambiente.
pub async fn sanitize_ask_user_arguments(
    ctx: &AppContext,
    conversation: &[AiMessage],
    arguments: &Value,
) -> Value {
    let Some(question) = arguments.get("question").and_then(Value::as_str) else {
        return arguments.clone();
    };
    let raw_options: Vec<String> = arguments
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let kind = asking_which_resource(question);
    let candidates = match kind {
        Some(k) => candidates_for_kind(ctx, conversation, k).await,
        None => Vec::new(),
    };

    let mut grounded_options: Vec<String> = Vec::new();
    for opt in raw_options {
        let in_conv = grounded_in_conversation(conversation, &opt);
        let in_candidates = option_matches_candidate(&opt, &candidates);
        let in_db = if !in_conv && !in_candidates {
            registered(&ctx.db, &opt).await.unwrap_or(false)
        } else {
            false
        };
        if in_conv || in_candidates || in_db {
            push_unique(&mut grounded_options, opt);
        } else if kind.is_none() && !looks_like_resource_identifier(&opt) {
            push_unique(&mut grounded_options, opt);
        }
    }

    if kind.is_some() {
        for candidate in candidates {
            if !option_matches_candidate(&candidate, &grounded_options) {
                push_unique(&mut grounded_options, candidate);
            }
        }
    }

    let mut updated = arguments.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert("options".to_string(), json!(grounded_options));
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> AiMessage {
        AiMessage {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    #[test]
    fn interno_e_o_que_so_existe_na_rede_do_usuario() {
        for alvo in [
            "10.0.0.1",
            "192.168.1.1",
            "172.16.4.2",
            "100.64.0.9",
            "169.254.1.1",
            "fd00::1",
            "[fe80::1]",
            "roteador",
            "nas.lan",
            "srv01.empresa.local",
        ] {
            assert!(is_internal(alvo), "{alvo}");
        }
        for alvo in [
            "8.8.8.8",
            "1.1.1.1",
            "google.com",
            "127.0.0.1",
            "localhost",
            "::1",
            "2001:4860:4860::8888",
        ] {
            assert!(!is_internal(alvo), "{alvo}");
        }
    }

    #[test]
    fn citacao_casa_so_a_palavra_inteira() {
        assert!(mentions("pinga o 10.0.0.1 agora", "10.0.0.1"));
        assert!(mentions("pinga o 10.0.0.1.", "10.0.0.1"));
        assert!(mentions(r#"{"ip":"10.0.0.1"}"#, "10.0.0.1"));
        assert!(mentions("snmp em 10.0.0.1:161", "10.0.0.1"));
        assert!(mentions("Testa o NAS.lan", "nas.lan"));
        assert!(!mentions("o 10.0.0.12 caiu", "10.0.0.1"));
        assert!(!mentions("o 110.0.0.1 caiu", "10.0.0.1"));
        assert!(!mentions("o 10.0.0.1.5 caiu", "10.0.0.1"));
        assert!(!mentions("qualquer coisa", ""));
    }

    #[test]
    fn origem_e_o_que_a_ia_leu_nao_o_que_ela_escreveu() {
        let conversa = [
            msg("system", "Você é o NetMonitor AI."),
            msg("user", "quais ping estão instáveis?"),
            msg("assistant", "Vou pingar 10.0.0.1"),
            msg(
                "tool",
                r#"{"devices":{"columns":["ip"],"rows":[["192.168.0.10"]]}}"#,
            ),
        ];
        assert!(!grounded_in_conversation(&conversa, "10.0.0.1"));
        assert!(grounded_in_conversation(&conversa, "192.168.0.10"));
    }

    #[test]
    fn identifica_pergunta_sobre_qual_recurso_e_ignora_pergunta_de_acao() {
        assert_eq!(
            asking_which_resource(
                "Qual container Docker você deseja agendar com a ação `docker_container_action`?"
            ),
            Some(ResourceKind::Container)
        );
        assert_eq!(
            asking_which_resource("Qual contêiner você quer reiniciar?"),
            Some(ResourceKind::Container)
        );
        assert_eq!(
            asking_which_resource("Qual dispositivo devo testar?"),
            Some(ResourceKind::Device)
        );
        assert_eq!(
            asking_which_resource("Qual alerta você deseja silenciar?"),
            Some(ResourceKind::Alert)
        );
        assert_eq!(
            asking_which_resource("Qual ação você deseja executar no container nginx?"),
            None
        );
        assert_eq!(
            asking_which_resource("O que você deseja fazer com os alertas ativos?"),
            None
        );
    }

    #[test]
    fn reconhece_identificadores_tecnicos_inventados_e_preserva_frases_naturais() {
        for slug in [
            "zabbix-proxy-01",
            "redis-fila-arnoldo",
            "nginx-recoisoir",
            "redis-fila",
            "#410 Queda de link",
            "sw-core-01",
            "10.0.0.50",
        ] {
            assert!(looks_like_resource_identifier(slug), "{slug}");
        }
        for natural in [
            "Apenas listar os alertas",
            "Silenciar alertas ativos",
            "Todos os dispositivos",
            "Reiniciar agora",
            "Segunda-feira",
        ] {
            assert!(!looks_like_resource_identifier(natural), "{natural}");
        }
    }

    #[test]
    fn extrai_containers_da_conversa_em_formato_tabela_ou_lista() {
        let conversa = [
            msg(
                "user",
                "<contexto>\n- Container Docker 'netmonitor-api' (id 123)\n</contexto>",
            ),
            msg(
                "tool",
                r#"{"containers":{"columns":["name","state"],"rows":[["postgres","running"],["redis","running"],["grafana","exited"]]}}"#,
            ),
        ];
        assert_eq!(
            containers_from_conversation(&conversa),
            vec!["postgres", "redis", "grafana", "netmonitor-api"]
        );
    }
}
