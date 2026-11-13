//! Exportação dos dados do assistente: as conversas, os comandos que a IA
//! deu e o que eles devolveram, num `.zip` para treinar um modelo e revisar
//! onde a IA errou e acertou.
//!
//! Cada parte tem um dono: [`signals`] lê erro e acerto de uma conversa,
//! [`training`] a converte para o formato de treino, [`repository`] consulta o
//! banco e [`bundle`] empacota. Aqui só se decide o que vai em cada arquivo.
//! O pacote é de todos os usuários — quem pede é o administrador (o
//! controller exige).

pub mod bundle;
pub mod repository;
pub mod signals;
pub mod training;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Duration, Utc};
use sea_orm::ConnectionTrait;
use serde::Serialize;
use serde_json::Value;

use self::{
    bundle::{Bundle, JsonLines},
    signals::{Outcome, Signals},
};
use crate::dtos::AiDataExportSummary;
use netmonitor_core::{
    models::{ai_conversations, plugin_runs},
    shared::errors::{AppError, AppResult},
};

/// Maior período aceito, em dias (dez anos: na prática, "tudo").
pub const MAX_DAYS: u32 = 3650;
/// Versão do formato do pacote; sobe quando um arquivo muda de forma.
pub const FORMAT_VERSION: u32 = 1;

const README: &str = include_str!("LEIAME.md");

/// Janela da exportação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub days: Option<u32>,
    pub since: Option<DateTime<Utc>>,
}

impl Period {
    /// Os últimos `days` dias a partir de `now`, ou tudo.
    ///
    /// # Errors
    ///
    /// `days` fora de `1..=MAX_DAYS`.
    pub fn last(days: Option<u32>, now: DateTime<Utc>) -> AppResult<Self> {
        match days {
            None => Ok(Self {
                days: None,
                since: None,
            }),
            Some(days @ 1..=MAX_DAYS) => Ok(Self {
                days: Some(days),
                since: Some(now - Duration::days(i64::from(days))),
            }),
            Some(_) => Err(AppError::validation(format!(
                "Período inválido: use de 1 a {MAX_DAYS} dias, ou deixe em branco para exportar tudo."
            ))),
        }
    }
}

/// O que a exportação levaria, sem ler as mensagens.
///
/// # Errors
///
/// Erro do banco.
pub async fn summary<C: ConnectionTrait>(db: &C, period: Period) -> AppResult<AiDataExportSummary> {
    let stats = repository::conversation_stats(db, period.since).await?;
    let users: BTreeSet<i64> = stats.iter().map(|(user, _, _)| *user).collect();
    let messages = stats
        .iter()
        .map(|(_, count, _)| u64::try_from(*count).unwrap_or(0))
        .sum();
    let oldest = stats.iter().map(|(_, _, updated)| updated).min();
    let newest = stats.iter().map(|(_, _, updated)| updated).max();
    Ok(AiDataExportSummary {
        conversations: stats.len() as u64,
        messages,
        users: users.len() as u64,
        device_runs: repository::count_ai_runs(db, period.since).await?,
        oldest: oldest.map(DateTime::to_rfc3339),
        newest: newest.map(DateTime::to_rfc3339),
    })
}

/// O pacote pronto para baixar.
#[derive(Debug)]
pub struct Export {
    pub filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Named<'a> {
    id: i64,
    name: Option<&'a str>,
}

impl<'a> Named<'a> {
    fn from(id: i64, names: &'a HashMap<i64, String>) -> Self {
        Self {
            id,
            name: names.get(&id).map(String::as_str),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationRecord<'a> {
    id: i64,
    title: &'a str,
    user: Named<'a>,
    created_at: String,
    updated_at: String,
    outcome: Outcome,
    signals: &'a Signals,
    messages: &'a [Value],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceRunRecord<'a> {
    id: i64,
    created_at: String,
    finished_at: Option<String>,
    device: Named<'a>,
    user: Option<Named<'a>>,
    plugin_id: Option<i64>,
    action: &'a str,
    status: &'a str,
    params: Option<&'a Value>,
    output: Option<&'a Value>,
    transcript: &'a Value,
    error: Option<&'a str>,
}

/// Somatório das conversas, para o manifesto.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Totals {
    conversations: usize,
    conversations_without_failures: usize,
    conversations_with_failures: usize,
    training_examples_without_failures: usize,
    training_examples_with_failures: usize,
    questions: usize,
    answers: usize,
    tool_calls: usize,
    tools_failed: usize,
    actions_declined: usize,
    answers_with_error: usize,
    rated_good: usize,
    rated_bad: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    device_runs: usize,
    /// Quantas vezes cada ferramenta falhou — por onde começar a corrigir.
    tool_failures: BTreeMap<String, usize>,
}

impl Totals {
    fn add(&mut self, signals: &Signals) {
        self.conversations += 1;
        match signals.outcome() {
            Outcome::NoFailures => self.conversations_without_failures += 1,
            Outcome::WithFailures => self.conversations_with_failures += 1,
        }
        self.questions += signals.questions;
        self.answers += signals.answers;
        self.tool_calls += signals.tool_calls;
        self.tools_failed += signals.tools_failed;
        self.actions_declined += signals.actions_declined;
        self.answers_with_error += signals.answers_with_error;
        self.rated_good += signals.rated_good;
        self.rated_bad += signals.rated_bad;
        for failure in &signals.failures {
            if let Some(tool) = &failure.tool {
                *self.tool_failures.entry(tool.clone()).or_default() += 1;
            }
        }
        self.prompt_tokens += signals.prompt_tokens;
        self.completion_tokens += signals.completion_tokens;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: &'static str,
    version: u32,
    generated_at: String,
    period_days: Option<u32>,
    since: Option<String>,
    totals: Totals,
    files: [&'static str; 6],
}

fn messages_of(row: &ai_conversations::Model) -> &[Value] {
    row.messages.as_array().map_or(&[], Vec::as_slice)
}

/// Exemplos de treino separados pela classificação automática.
#[derive(Default)]
struct TrainingSets {
    without_failures: JsonLines,
    with_failures: JsonLines,
}

/// Grava as conversas do período direto no pacote (página a página) e
/// separa os exemplos de treino.
async fn write_conversations<C: ConnectionTrait>(
    db: &C,
    period: Period,
    bundle: &mut Bundle,
    totals: &mut Totals,
) -> AppResult<TrainingSets> {
    let users = repository::user_names(db).await?;
    let mut sets = TrainingSets::default();
    bundle.begin("conversas.jsonl")?;
    let mut page = 0;
    loop {
        let rows = repository::conversation_page(db, period.since, page).await?;
        for row in &rows {
            let messages = messages_of(row);
            let signals = signals::analyze(messages);
            totals.add(&signals);
            bundle.write_line(&ConversationRecord {
                id: row.id,
                title: &row.title,
                user: Named::from(row.user_id, &users),
                created_at: row.created_at.to_rfc3339(),
                updated_at: row.updated_at.to_rfc3339(),
                outcome: signals.outcome(),
                signals: &signals,
                messages,
            })?;
            if let Some(example) = training::to_chat_example(messages) {
                match signals.outcome() {
                    Outcome::NoFailures => sets.without_failures.push(&example)?,
                    Outcome::WithFailures => sets.with_failures.push(&example)?,
                }
            }
        }
        if (rows.len() as u64) < repository::PAGE_SIZE {
            break;
        }
        page += 1;
    }
    totals.training_examples_without_failures = sets.without_failures.len();
    totals.training_examples_with_failures = sets.with_failures.len();
    Ok(sets)
}

async fn device_runs<C: ConnectionTrait>(db: &C, period: Period) -> AppResult<JsonLines> {
    let runs: Vec<plugin_runs::Model> = repository::ai_runs(db, period.since).await?;
    let device_ids: BTreeSet<i64> = runs.iter().map(|run| run.device_id).collect();
    let devices = repository::device_names(db, device_ids.into_iter().collect()).await?;
    let users = repository::user_names(db).await?;
    let mut lines = JsonLines::default();
    for run in &runs {
        lines.push(&DeviceRunRecord {
            id: run.id,
            created_at: run.created_at.to_rfc3339(),
            finished_at: run.finished_at.map(|at| at.to_rfc3339()),
            device: Named::from(run.device_id, &devices),
            user: run.user_id.map(|id| Named::from(id, &users)),
            plugin_id: run.plugin_id,
            action: &run.action,
            status: &run.status,
            params: run.params.as_ref(),
            output: run.output.as_ref(),
            transcript: &run.transcript,
            error: run.error.as_deref(),
        })?;
    }
    Ok(lines)
}

/// Monta o pacote do período.
///
/// # Errors
///
/// Erro do banco ou do compressor.
pub async fn export<C: ConnectionTrait>(
    db: &C,
    period: Period,
    now: DateTime<Utc>,
) -> AppResult<Export> {
    let mut bundle = Bundle::new();
    let mut totals = Totals::default();
    bundle.add("LEIAME.md", README.as_bytes())?;

    let sets = write_conversations(db, period, &mut bundle, &mut totals).await?;
    bundle.add(
        "treino/sem-falhas.jsonl",
        &sets.without_failures.into_bytes(),
    )?;
    bundle.add("treino/com-falhas.jsonl", &sets.with_failures.into_bytes())?;

    let runs = device_runs(db, period).await?;
    totals.device_runs = runs.len();
    bundle.add("execucoes-em-equipamentos.jsonl", &runs.into_bytes())?;

    bundle.add_json(
        "manifesto.json",
        &Manifest {
            format: "netmonitor-ai-data",
            version: FORMAT_VERSION,
            generated_at: now.to_rfc3339(),
            period_days: period.days,
            since: period.since.map(|since| since.to_rfc3339()),
            totals,
            files: [
                "LEIAME.md",
                "manifesto.json",
                "conversas.jsonl",
                "treino/sem-falhas.jsonl",
                "treino/com-falhas.jsonl",
                "execucoes-em-equipamentos.jsonl",
            ],
        },
    )?;

    Ok(Export {
        filename: filename(now),
        bytes: bundle.finish()?,
    })
}

/// Nome com carimbo de data, para não acumular `(1)`, `(2)` nos downloads.
#[must_use]
pub fn filename(now: DateTime<Utc>) -> String {
    format!("netmonitor-ia-dados-{}.zip", now.format("%Y%m%d-%H%M%S"))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn agora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 12, 15, 4, 5).unwrap()
    }

    #[test]
    fn periodo_vazio_e_tudo_e_fora_da_faixa_e_recusado() {
        assert_eq!(Period::last(None, agora()).unwrap().since, None);
        let semana = Period::last(Some(7), agora()).unwrap();
        assert_eq!(semana.since, Some(agora() - Duration::days(7)));
        assert!(Period::last(Some(0), agora()).is_err());
        assert!(Period::last(Some(MAX_DAYS + 1), agora()).is_err());
    }

    #[test]
    fn nome_do_arquivo_tem_carimbo() {
        assert_eq!(filename(agora()), "netmonitor-ia-dados-20261112-150405.zip");
    }
}
