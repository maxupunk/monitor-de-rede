//! Alvo de teste ativo precisa ter origem.
//!
//! Sem dado nenhum, um modelo pequeno "propõe" um ping em `10.0.0.1`,
//! `10.0.0.2` e `10.0.0.3` — endereços que ele não tem como conhecer — e o
//! usuário recebe três pedidos de confirmação inúteis (ou três testes contra
//! nada, sem a confirmação). Endereço interno só é testado se veio da
//! conversa (o usuário escreveu, uma consulta devolveu, o administrador pôs no
//! prompt) ou está cadastrado no inventário. Alvo público (`8.8.8.8`,
//! `google.com`) é conhecimento geral e segue livre, como o loopback.

use std::{net::IpAddr, ops::Range, sync::LazyLock};

use loco_rs::prelude::AppContext;
use regex::{Regex, RegexBuilder};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::drivers::traits::AiMessage;
use netmonitor_core::{
    alerts::contracts::AlertStatus,
    models::{_entities::alert_events, alert_rules, devices, monitors, scheduled_tasks},
    shared::errors::AppResult,
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
        // A conferência repete o que a IA inventou: não é origem.
        .filter(|content| !content.starts_with(ANSWER_CHECK_TAG))
        .any(|content| mentions(content, target))
}

/// Fotografia leve do inventário carregada uma única vez por verificação,
/// projetando só as colunas de nome/IP/alvo em vez de todas as colunas.
struct InventorySnapshot {
    devices: Vec<(String, Option<String>)>,
    monitors: Vec<(String, Value)>,
}

impl InventorySnapshot {
    async fn load<C: ConnectionTrait>(db: &C) -> AppResult<Self> {
        let devices = devices::Entity::find()
            .select_only()
            .columns([devices::Column::Name, devices::Column::IpAddress])
            .into_tuple()
            .all(db)
            .await?;
        let monitors = monitors::Entity::find()
            .select_only()
            .columns([monitors::Column::Name, monitors::Column::Configuration])
            .into_tuple()
            .all(db)
            .await?;
        Ok(Self { devices, monitors })
    }

    fn contains(&self, target: &str) -> bool {
        let wanted = normalize(target);
        if wanted.is_empty() {
            return false;
        }
        let same = |value: &str| {
            let value = normalize(value);
            value == wanted || mentions(&value, &wanted)
        };
        if self
            .devices
            .iter()
            .any(|(name, ip)| same(name) || ip.as_deref().is_some_and(same))
        {
            return true;
        }
        self.monitors.iter().any(|(name, config)| {
            same(name) || super::tools::monitor_target(config).is_some_and(same)
        })
    }
}

/// O alvo é o IP de um dispositivo ou o alvo de um monitor cadastrado (o
/// resultado da consulta pode ter sido podado da conversa).
async fn registered<C: ConnectionTrait>(db: &C, target: &str) -> AppResult<bool> {
    Ok(InventorySnapshot::load(db).await?.contains(target))
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
            if let Ok(central) = crate::central::Central::from_context(ctx) {
                for name in central.container_names(ctx).await {
                    push_unique(&mut out, name);
                }
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
                        Some(msg) if !msg.trim().is_empty() => {
                            format!("#{} {}", row.id, msg.trim())
                        }
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

    let mut inventory: Option<Option<InventorySnapshot>> = None;
    let mut grounded_options: Vec<String> = Vec::new();
    for opt in raw_options {
        let in_conv = grounded_in_conversation(conversation, &opt);
        let in_candidates = option_matches_candidate(&opt, &candidates);
        let in_db = if !in_conv && !in_candidates {
            if inventory.is_none() {
                inventory = Some(InventorySnapshot::load(&ctx.db).await.ok());
            }
            inventory
                .as_ref()
                .and_then(Option::as_ref)
                .is_some_and(|snapshot| snapshot.contains(&opt))
        } else {
            false
        };
        // Fica a opção ancorada (conversa, candidatos ou banco) e, sem tipo de
        // recurso, a que nem parece identificador de recurso.
        let free_text = kind.is_none() && !looks_like_resource_identifier(&opt);
        if in_conv || in_candidates || in_db || free_text {
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

/// Abre a mensagem com que o harness devolve à IA o que ela inventou.
pub const ANSWER_CHECK_TAG: &str = "<conferencia>";

/// Até quantos identificadores sem origem a conferência da resposta aponta.
const MAX_UNSUPPORTED: usize = 5;

/// O que a resposta final cita como recurso sem que nada nesta sessão o tenha
/// mostrado: entre crases (`agente-west-01`, `wlan-sala`) ou um IP interno
/// solto. "Mostrado" é o pedido, o `<contexto>`, o system prompt ou um
/// resultado de consulta — nunca o que a própria IA escreveu. `is_tool`
/// separa os nomes de ferramenta, que a IA conhece por definição.
#[must_use]
pub fn unsupported_identifiers(
    answer: &str,
    conversation: &[AiMessage],
    earlier: usize,
    is_tool: impl Fn(&str) -> bool,
) -> Vec<String> {
    let quoted = answer
        .split('`')
        .skip(1)
        .step_by(2)
        // Bloco de código (```) e trecho com espaço: cada palavra é candidata.
        .flat_map(str::split_whitespace)
        // `get_docker_containers(host=x)` é a ferramenta; `host=x` é o `x`.
        .map(|span| span.split('(').next().unwrap_or(span))
        .map(|span| span.rsplit('=').next().unwrap_or(span))
        .map(|span| span.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|span| {
            !span.is_empty()
                && looks_like_resource_identifier(span)
                && !is_constant_or_quantity(span)
        });
    let loose = answer
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '`' | '*' | '='))
        .map(|token| token.trim_end_matches(['.', ':']))
        .filter(|token| {
            token.contains('.') && token.parse::<IpAddr>().is_ok() && is_internal(token)
        });
    let mut found: Vec<String> = Vec::new();
    for candidate in quoted.chain(loose) {
        if found.len() >= MAX_UNSUPPORTED {
            break;
        }
        if !is_tool(candidate) && !grounded(conversation, earlier, candidate) {
            push_unique(&mut found, candidate);
        }
    }
    found
}

/// O nome tem origem: qualquer mensagem antes da pergunta atual (as respostas
/// anteriores já foram conferidas na vez delas, e os resultados de consulta
/// daquelas rodadas não voltam no histórico) ou, desta vez, algo que não foi
/// a IA que escreveu.
fn grounded(conversation: &[AiMessage], earlier: usize, target: &str) -> bool {
    let (before, now) = conversation.split_at(earlier.min(conversation.len()));
    before
        .iter()
        .filter_map(|message| message.content.as_deref())
        .any(|content| mentions(content, target))
        || grounded_in_conversation(now, target)
}

/// Constante (`OLLAMA_CONTEXT_LENGTH`) ou grandeza (`24h`, `5GHz`, `2.4GHz`):
/// tem cara de identificador, mas não é recurso da rede.
fn is_constant_or_quantity(span: &str) -> bool {
    let constant = span.chars().any(char::is_alphabetic)
        && span
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    let unit = span.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ',');
    let quantity = unit.len() < span.len()
        && (1..=4).contains(&unit.len())
        && unit.chars().all(|c| c.is_ascii_alphabetic());
    constant || quantity
}

/// O que fica no lugar de um nome inventado que a IA insistiu em citar.
pub const UNCONFIRMED: &str = "[não confirmado]";

/// Troca cada nome inventado por [`UNCONFIRMED`], sem diferenciar caixa e só
/// onde ele aparece inteiro (`10.0.0.1` não mexe em `10.0.0.10`).
#[must_use]
pub fn redact(answer: &str, names: &[String]) -> String {
    let mut text = answer.to_string();
    for name in names.iter().filter(|name| !name.is_empty()) {
        let Ok(re) = RegexBuilder::new(&regex::escape(name))
            .case_insensitive(true)
            .build()
        else {
            continue;
        };
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for found in re.find_iter(&text) {
            let start = found.start();
            let end = found.end();
            let before = text[..start].chars().next_back();
            let after = text[end..].chars().next();
            let whole = before.is_none_or(|c| !continues_host(c))
                && after.is_none_or(|c| !continues_host(c));
            if whole && start >= last {
                out.push_str(&text[last..start]);
                out.push_str(UNCONFIRMED);
                last = end;
            }
        }
        out.push_str(&text[last..]);
        text = out;
    }
    text
}

/// Os resultados de consulta que a IA leu nesta resposta, guardados antes da
/// poda: o que saiu da conversa para caber na janela (ou foi trocado por uma
/// consulta repetida) continua valendo como origem de um nome.
#[derive(Debug, Default)]
pub struct SeenResults {
    texts: Vec<String>,
    recorded: usize,
}

impl SeenResults {
    /// Guarda os resultados que entraram na conversa desde a última vez. Vem
    /// antes de qualquer poda da rodada: só as mensagens novas são lidas.
    pub fn record(&mut self, conversation: &[AiMessage]) {
        let fresh = conversation.iter().skip(self.recorded);
        self.texts.extend(
            fresh
                .filter(|message| message.role == "tool")
                .filter_map(|message| message.content.clone()),
        );
        self.recorded = conversation.len();
    }

    fn mentions(&self, target: &str) -> bool {
        self.texts.iter().any(|text| mentions(text, target))
    }
}

/// [`unsupported_identifiers`] menos o que algum resultado desta resposta
/// mostrou (mesmo podado depois) e o que está cadastrado no inventário.
pub async fn invented_in_answer<C: ConnectionTrait>(
    db: &C,
    answer: &str,
    conversation: &[AiMessage],
    earlier: usize,
    seen: &SeenResults,
    is_tool: impl Fn(&str) -> bool,
) -> Vec<String> {
    let candidates: Vec<String> = unsupported_identifiers(answer, conversation, earlier, is_tool)
        .into_iter()
        .filter(|candidate| !seen.mentions(candidate))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let snapshot = InventorySnapshot::load(db).await.ok();
    candidates
        .into_iter()
        .filter(|candidate| !snapshot.as_ref().is_some_and(|inv| inv.contains(candidate)))
        .collect()
}

/// A chave do aviso da conferência na tela: cada estado substitui o anterior.
pub const ANSWER_CHECK_NOTICE: &str = "answer-check";

/// O desfecho quando a versão conferida saiu limpa.
pub const ANSWER_CHECKED: &str = "Resposta conferida: a IA refez o que a conferência apontou.";

/// O que fica no lugar de um gráfico que a IA desenhou em texto.
pub const NO_CHART: &str = "[gráfico indisponível]";

/// Abertura ou fechamento de uma tag qualquer (`<analysis_chart data=…>`,
/// `</chart>`, `<grafico/>`).
static TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(/?)\s*(\p{L}[\w:.-]*)[^<>]*?(/?)>").expect("regex de tag"));

/// A tag tem nome de gráfico: modelo pequeno inventa `analysis_chart`,
/// `line-graph`, `grafico_cpu`, `plot`.
fn is_chart_tag(name: &str) -> bool {
    let name = name.to_lowercase();
    ["chart", "graph", "grafico", "gráfico", "plot"]
        .iter()
        .any(|word| name.contains(word))
}

/// Os trechos em que a IA "desenha" um gráfico com uma tag inventada
/// (`<analysis_chart data="[…]">…</analysis_chart>`). Gráfico só sai por
/// ferramenta `chart_*`: a tela mostraria a tag crua, com números que nenhuma
/// consulta trouxe. O par aberto/fechado vira um trecho só, com o miolo; a
/// tag sem par sai sozinha.
#[must_use]
pub fn fake_charts(answer: &str) -> Vec<Range<usize>> {
    let mut spans: Vec<Range<usize>> = Vec::new();
    // Nome e trecho da abertura que espera o fechamento.
    let mut open: Option<(&str, Range<usize>)> = None;
    for tag in TAG.captures_iter(answer) {
        let (Some(whole), Some(name)) = (tag.get(0), tag.get(2)) else {
            continue;
        };
        if !is_chart_tag(name.as_str()) {
            continue;
        }
        let closing = !tag[1].is_empty();
        let self_closing = !tag[3].is_empty();
        if closing {
            match open.take() {
                Some((opened, at)) if opened.eq_ignore_ascii_case(name.as_str()) => {
                    spans.push(at.start..whole.end());
                }
                other => {
                    open = other;
                    spans.push(whole.range());
                }
            }
        } else if self_closing {
            spans.push(whole.range());
        } else if let Some((_, at)) = open.replace((name.as_str(), whole.range())) {
            spans.push(at);
        }
    }
    if let Some((_, at)) = open {
        spans.push(at);
    }
    spans.sort_by_key(|span| span.start);
    // Um fechamento solto dentro de um par já está no trecho do par.
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start < last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

/// Troca cada gráfico em texto por [`NO_CHART`].
fn redact_charts(answer: &str) -> String {
    let mut text = String::with_capacity(answer.len());
    let mut last = 0;
    for span in fake_charts(answer) {
        text.push_str(&answer[last..span.start]);
        text.push_str(NO_CHART);
        last = span.end;
    }
    text.push_str(&answer[last..]);
    text
}

/// O que a conferência acha na resposta final: nomes sem origem e gráfico
/// desenhado em texto. Os textos para a tela e para a IA moram aqui, para o
/// laço do agente só perguntar, pedir a correção e redigir.
#[derive(Debug, Default)]
pub struct AnswerCheck {
    invented: Vec<String>,
    fake_chart: bool,
    /// Nomes de ferramenta (`chart_interface_traffic`) que a resposta cita
    /// sem o usuário ter escrito: jargão que não diz nada a quem lê.
    internal: Vec<String>,
}

/// Os nomes de ferramenta que a resposta cita e a pergunta não: modelo
/// pequeno repete o erro que o harness lhe devolveu ("use uma destas:
/// `chart_…`") como se fosse resposta. Todo nome do catálogo tem `_`.
fn internal_names(answer: &str, question: &str, is_tool: impl Fn(&str) -> bool) -> Vec<String> {
    let mut found = Vec::new();
    for token in answer.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if found.len() >= MAX_UNSUPPORTED {
            break;
        }
        if token.contains('_') && is_tool(token) && !question.contains(token) {
            push_unique(&mut found, token);
        }
    }
    found
}

impl AnswerCheck {
    /// Confere a resposta. Ver [`invented_in_answer`] e [`fake_charts`].
    pub async fn run<C: ConnectionTrait>(
        db: &C,
        answer: &str,
        conversation: &[AiMessage],
        earlier: usize,
        seen: &SeenResults,
        is_tool: impl Fn(&str) -> bool,
    ) -> Self {
        let question = conversation
            .get(earlier)
            .and_then(|message| message.content.as_deref())
            .unwrap_or_default();
        Self {
            internal: internal_names(answer, question, &is_tool),
            invented: invented_in_answer(db, answer, conversation, earlier, seen, &is_tool).await,
            fake_chart: !fake_charts(answer).is_empty(),
        }
    }

    #[must_use]
    pub fn is_clean(&self) -> bool {
        !self.needs_redaction() && self.internal.is_empty()
    }

    /// O que não pode ficar na tela mesmo depois da correção: nome inventado
    /// e gráfico falso enganam. Jargão só confunde — fica, se a IA insistir.
    #[must_use]
    pub fn needs_redaction(&self) -> bool {
        !self.invented.is_empty() || self.fake_chart
    }

    /// O aviso enquanto a IA confere a resposta.
    #[must_use]
    pub fn retry_notice(&self) -> String {
        let mut problems = Vec::new();
        if !self.invented.is_empty() {
            problems.push(format!(
                "{} não apareceu em nenhuma consulta",
                self.invented.join(", ")
            ));
        }
        if self.fake_chart {
            problems.push("o gráfico veio escrito em texto, sem dados de consulta".to_string());
        }
        if !self.internal.is_empty() {
            problems.push("a resposta citava nomes internos de ferramentas".to_string());
        }
        format!(
            "Conferindo a resposta: {}; a IA está verificando nos dados.",
            problems.join("; ")
        )
    }

    /// O pedido de correção devolvido à IA.
    #[must_use]
    pub fn retry_request(&self) -> String {
        let mut request = String::from(ANSWER_CHECK_TAG);
        if !self.invented.is_empty() {
            request.push_str(&format!(
                "Sua resposta cita {}, que não veio de nenhuma consulta nem do pedido. Não invente recursos: consulte com as ferramentas. ",
                self.invented.join(", ")
            ));
        }
        if self.fake_chart {
            request.push_str(
                "Sua resposta desenha um gráfico com uma tag de texto, que a tela não desenha. Gráfico só sai por uma ferramenta chart_* (grupo charts); se nenhuma serve para esse dado, use uma tabela curta só com números que uma consulta trouxe. ",
            );
        }
        if !self.internal.is_empty() {
            request.push_str(&format!(
                "Sua resposta cita nomes internos de ferramentas ({}), que o usuário não conhece. Diga em linguagem simples o que dá e o que não dá para mostrar, sem nomes de ferramentas. ",
                self.internal.join(", ")
            ));
        }
        request.push_str("Responda de novo, curto, ou diga que não encontrou.</conferencia>");
        request
    }

    /// O aviso quando a IA insiste no que a conferência apontou.
    #[must_use]
    pub fn final_notice(&self) -> String {
        let mut parts = Vec::new();
        if !self.invented.is_empty() {
            parts.push(format!(
                "A IA citou nomes que não existem em nenhum dado consultado; eles aparecem como {UNCONFIRMED}."
            ));
        }
        if self.fake_chart {
            parts.push(format!(
                "A IA escreveu um gráfico em texto, sem uma ferramenta de gráfico para esse dado; ele aparece como {NO_CHART}."
            ));
        }
        parts.join(" ")
    }

    /// A resposta com os nomes inventados e os gráficos em texto trocados.
    #[must_use]
    pub fn redact(&self, answer: &str) -> String {
        let named = redact(answer, &self.invented);
        if self.fake_chart {
            redact_charts(&named)
        } else {
            named
        }
    }
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

    fn conversa(textos: &[(&str, &str)]) -> Vec<AiMessage> {
        textos
            .iter()
            .map(|(role, content)| AiMessage {
                role: (*role).to_string(),
                content: Some((*content).to_string()),
                tool_calls: None,
                tool_call_id: None,
            })
            .collect()
    }

    #[test]
    fn resposta_que_cita_recurso_sem_origem_e_apontada() {
        let sessao = conversa(&[
            ("system", "Você é o assistente."),
            ("user", "o container postgres parado é grave?"),
        ]);
        let resposta = "Hosts conectados (`get_docker_hosts`): `central`, `agente-west-01`,                         `agente-east-01`. O gateway 10.0.0.77 também.";
        let ferramenta = |nome: &str| nome == "get_docker_hosts";
        assert_eq!(
            unsupported_identifiers(resposta, &sessao, 0, ferramenta),
            vec!["agente-west-01", "agente-east-01", "10.0.0.77"]
        );
    }

    #[test]
    fn o_que_veio_de_consulta_ou_do_pedido_nao_e_apontado() {
        let sessao = conversa(&[
            ("user", "e o ap-sala?"),
            (
                "tool",
                r#"{"rows":[["agente-west-01","online"],["10.0.0.77"]]}"#,
            ),
            ("assistant", "`inventado-pela-ia-01`"),
        ]);
        let resposta =
            "O `ap-sala` está no `agente-west-01` (10.0.0.77). Ver `OLLAMA_CONTEXT_LENGTH`?";
        assert!(
            unsupported_identifiers(resposta, &sessao, 0, |_| false).is_empty(),
            "o que veio do pedido ou de consulta, constante e grandeza não são apontados"
        );
        // O que só a própria IA escreveu antes não ancora nada.
        assert_eq!(
            unsupported_identifiers("De novo: `inventado-pela-ia-01`.", &sessao, 0, |_| false),
            vec!["inventado-pela-ia-01"]
        );
        let livre = "Em `24h`, `5GHz` e `2.4GHz` sem erros; agente-west-02 sem crase e IPv4.";
        assert!(unsupported_identifiers(livre, &sessao, 0, |_| false).is_empty());
    }

    #[test]
    fn resposta_de_pergunta_anterior_e_chamada_de_ferramenta_tem_origem() {
        let sessao = conversa(&[
            ("system", "Você é o assistente."),
            ("user", "quais hosts docker?"),
            ("assistant", "O `agente-norte-01` (10.0.0.9) está online."),
            ("user", "e ele está bem?"),
        ]);
        let ferramenta = |nome: &str| nome == "get_docker_containers";
        let resposta = "Sim: `get_docker_containers(host=agente-norte-01)` mostra o                         `agente-norte-01` em 10.0.0.9; o `agente-sul-02` não.";
        assert_eq!(
            unsupported_identifiers(resposta, &sessao, 3, ferramenta),
            vec!["agente-sul-02"],
            "a resposta anterior vale como origem; a desta vez, não"
        );
        assert_eq!(
            unsupported_identifiers(resposta, &sessao, 0, ferramenta),
            vec!["agente-norte-01", "agente-sul-02", "10.0.0.9"]
        );
    }

    #[test]
    fn resultado_podado_continua_valendo_como_origem() {
        let mut sessao = conversa(&[
            ("user", "quais containers?"),
            ("tool", r#"{"rows":[["grafana-01","running"]]}"#),
        ]);
        let mut vistos = SeenResults::default();
        vistos.record(&sessao);
        sessao[1].content = Some(super::super::compaction::PRUNED_NOTICE.to_string());
        sessao.extend(conversa(&[("tool", r#"{"rows":[["loki-02"]]}"#)]));
        vistos.record(&sessao);
        assert!(vistos.mentions("grafana-01"), "lido antes da poda");
        assert!(vistos.mentions("loki-02"), "o que chegou depois também");
        assert!(!vistos.mentions("tempo-03"));
        assert_eq!(vistos.texts.len(), 2, "cada resultado entra uma vez só");
    }

    #[test]
    fn nome_inventado_some_da_resposta_so_onde_aparece_inteiro() {
        let nomes = vec!["agente-west-01".to_string(), "10.0.0.1".to_string()];
        assert_eq!(
            redact(
                "No `agente-west-01` e no `Agente-West-01` (10.0.0.1); o 10.0.0.10 e o agente-west-012 ficam.",
                &nomes
            ),
            "No `[não confirmado]` e no `[não confirmado]` ([não confirmado]); o 10.0.0.10 e o agente-west-012 ficam."
        );
    }

    #[test]
    fn nome_em_bloco_de_codigo_tambem_e_conferido() {
        let sessao = conversa(&[("user", "o postgres parado é grave?")]);
        let resposta = "Containers:
```
- nginx-proxy: parado (exited)
- postgres: parado
```
                        Veja `get_docker_hosts result`.";
        assert_eq!(
            unsupported_identifiers(resposta, &sessao, 0, |nome| nome == "get_docker_hosts"),
            vec!["nginx-proxy"]
        );
    }

    #[test]
    fn grafico_em_tag_inventada_e_achado_com_o_miolo() {
        let resposta = "CPU por hora:
<analysis_chart data=\"[12, 40, 7]\">
Pico às 14h
</analysis_chart>
Fim.";
        let trechos = fake_charts(resposta);
        assert_eq!(trechos.len(), 1, "{trechos:?}");
        assert_eq!(
            &resposta[trechos[0].clone()],
            "<analysis_chart data=\"[12, 40, 7]\">
Pico às 14h
</analysis_chart>"
        );
        assert_eq!(
            redact_charts(resposta),
            format!(
                "CPU por hora:
{NO_CHART}
Fim."
            )
        );
    }

    #[test]
    fn grafico_sem_par_ou_autofechado_sai_sozinho() {
        assert_eq!(
            redact_charts("Veja <line-graph values=\"1,2\"/> e <Grafico_CPU tipo=linha> ok"),
            format!("Veja {NO_CHART} e {NO_CHART} ok")
        );
        assert_eq!(
            redact_charts("antes </chart> depois"),
            format!("antes {NO_CHART} depois")
        );
    }

    #[test]
    fn texto_comum_com_sinal_de_menor_nao_e_grafico() {
        for resposta in [
            "Latência < 5 ms e perda > 2%.",
            "Use <b>negrito</b> e `<table>` sem medo.",
            "O gráfico da tela mostra a CPU.",
        ] {
            assert!(fake_charts(resposta).is_empty(), "{resposta}");
        }
    }

    #[test]
    fn nome_de_ferramenta_na_resposta_e_jargao_salvo_se_o_usuario_escreveu() {
        let ferramenta = |nome: &str| nome.starts_with("chart_") || nome == "list_devices";
        assert_eq!(
            internal_names(
                "Use `chart_device_metric` ou chart_logs_volume; list_devices também.",
                "o que o list_devices mostra?",
                ferramenta
            ),
            vec!["chart_device_metric", "chart_logs_volume"]
        );
        assert!(internal_names("Tudo certo no snake_case.", "oi", ferramenta).is_empty());

        let so_jargao = AnswerCheck {
            internal: vec!["chart_device_metric".into()],
            ..AnswerCheck::default()
        };
        assert!(
            !so_jargao.is_clean(),
            "pede uma rodada para explicar sem jargão"
        );
        assert!(
            !so_jargao.needs_redaction(),
            "mas não apaga nada se a IA insistir"
        );
        assert!(so_jargao.retry_request().contains("linguagem simples"));
    }

    #[test]
    fn conferencia_junta_nome_inventado_e_grafico_em_texto() {
        let conferencia = AnswerCheck {
            invented: vec!["agente-west-01".into()],
            fake_chart: true,
            internal: Vec::new(),
        };
        assert!(!conferencia.is_clean());
        let pedido = conferencia.retry_request();
        assert!(pedido.starts_with(ANSWER_CHECK_TAG));
        assert!(pedido.contains("agente-west-01") && pedido.contains("chart_*"));
        assert!(conferencia.retry_notice().contains("gráfico"));
        assert_eq!(
            conferencia.redact("No `agente-west-01`: <chart/>"),
            format!("No `{UNCONFIRMED}`: {NO_CHART}")
        );
        assert!(AnswerCheck::default().is_clean());
    }
}
