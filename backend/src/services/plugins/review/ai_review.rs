//! A leitura do plugin pela IA configurada.
//!
//! Uma chamada só, **sem ferramentas**: a IA lê e opina, não executa nada. O
//! pedido é de JSON estruturado; resposta que não parseia vira erro no
//! relatório ("revisão por IA indisponível"), nunca um "parece ok" implícito.

use std::time::Duration;

use futures::StreamExt;
use sea_orm::ConnectionTrait;
use serde::Deserialize;

use super::{AiReview, Finding, Severity};
use crate::services::{
    ai::{
        drivers::{
            self,
            traits::{AiChatOptions, AiDriver, AiMessage},
        },
        settings,
    },
    plugins::package::PluginPackage,
    shared::text::truncate_chars,
};

/// O prompt do revisor, versionado junto com o código.
pub const REVIEWER_PROMPT: &str = include_str!("../../ai/knowledge/plugin_security_review.md");

const MAX_SCRIPT_CHARS: usize = 60_000;
const REVIEW_TIMEOUT: Duration = Duration::from_secs(120);
const REVIEW_MAX_TOKENS: u32 = 2_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFinding {
    #[serde(default)]
    severity: Option<String>,
    message: String,
    #[serde(default)]
    line: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReview {
    risk_level: String,
    summary: String,
    #[serde(default)]
    findings: Vec<RawFinding>,
    #[serde(default)]
    mismatch_with_usage: bool,
}

fn severity(value: &str) -> Severity {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" | "critico" | "crítico" => Severity::Critical,
        "high" | "alto" => Severity::High,
        "medium" | "medio" | "médio" => Severity::Medium,
        "info" => Severity::Info,
        _ => Severity::Low,
    }
}

/// O que vai para a IA ler.
#[must_use]
pub fn review_input(package: &PluginPackage, static_findings: &[Finding]) -> String {
    let manifest = serde_json::to_string_pretty(&package.manifest).unwrap_or_default();
    let findings = if static_findings.is_empty() {
        "nenhum".to_owned()
    } else {
        static_findings
            .iter()
            .map(|f| format!("- [{:?}] {} (linha {:?})", f.severity, f.message, f.line))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "## Manifesto\n```json\n{manifest}\n```\n\n## Uso declarado\n{}\n\n## Script (Rhai)\n```rhai\n{}\n```\n\n## Achados da análise estática\n{findings}\n",
        truncate_chars(&package.usage, 8_000),
        truncate_chars(&package.script, MAX_SCRIPT_CHARS),
    )
}

/// Extrai e interpreta o JSON da resposta.
///
/// # Errors
///
/// Resposta sem objeto JSON ou fora do formato pedido.
pub fn parse_review(answer: &str, model: Option<String>) -> Result<AiReview, String> {
    let start = answer.find('{').ok_or("a IA não devolveu JSON")?;
    let end = answer.rfind('}').ok_or("a IA não devolveu JSON")?;
    if end < start {
        return Err("a IA não devolveu JSON".into());
    }
    let raw: RawReview = serde_json::from_str(&answer[start..=end])
        .map_err(|error| format!("resposta da IA fora do formato: {error}"))?;
    Ok(AiReview {
        risk_level: severity(&raw.risk_level),
        summary: truncate_chars(raw.summary.trim(), 2_000),
        findings: raw
            .findings
            .into_iter()
            .take(30)
            .map(|finding| Finding {
                severity: severity(finding.severity.as_deref().unwrap_or("low")),
                rule: "ai".into(),
                message: truncate_chars(&finding.message, 500),
                line: finding.line,
                excerpt: None,
            })
            .collect(),
        mismatch_with_usage: raw.mismatch_with_usage,
        model,
    })
}

/// Pede a revisão ao driver informado.
///
/// # Errors
///
/// Provedor indisponível, tempo esgotado ou resposta inválida.
pub async fn review_with(
    driver: &dyn AiDriver,
    package: &PluginPackage,
    static_findings: &[Finding],
) -> Result<AiReview, String> {
    let messages = vec![
        AiMessage {
            role: "system".into(),
            content: Some(REVIEWER_PROMPT.into()),
            tool_calls: None,
            tool_call_id: None,
        },
        AiMessage {
            role: "user".into(),
            content: Some(review_input(package, static_findings)),
            tool_calls: None,
            tool_call_id: None,
        },
    ];
    let work = async {
        let mut stream = driver
            .chat_stream(
                &messages,
                &[],
                AiChatOptions {
                    max_tokens: Some(REVIEW_MAX_TOKENS),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        let (mut answer, mut model) = (String::new(), driver.model().map(str::to_owned));
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;
            if let Some(delta) = chunk.text_delta {
                answer.push_str(&delta);
            }
            if chunk.model.is_some() {
                model = chunk.model;
            }
        }
        parse_review(&answer, model)
    };
    tokio::time::timeout(REVIEW_TIMEOUT, work)
        .await
        .map_err(|_| "a IA não respondeu a tempo".to_owned())?
}

/// Revisão com a IA configurada no sistema.
///
/// # Errors
///
/// IA desligada, sem configuração válida ou falha da chamada — a mensagem vai
/// para o relatório.
pub async fn review<C: ConnectionTrait>(
    db: &C,
    package: &PluginPackage,
    static_findings: &[Finding],
) -> Result<AiReview, String> {
    let settings = settings::load(db)
        .await
        .map_err(|error| error.to_string())?;
    if !settings.enabled {
        return Err("o Assistente IA está desligado; a revisão foi só estática".into());
    }
    let driver = drivers::create_driver(&settings).map_err(|error| error.to_string())?;
    review_with(driver.as_ref(), package, static_findings).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resposta_com_texto_em_volta_e_aceita() {
        let answer = "Segue:\n```json\n{\"riskLevel\":\"high\",\"summary\":\"troca a senha\",\
                      \"findings\":[{\"severity\":\"high\",\"message\":\"passwd\",\"line\":3}],\
                      \"mismatchWithUsage\":true}\n```";
        let review = parse_review(answer, Some("m".into())).unwrap();
        assert_eq!(review.risk_level, Severity::High);
        assert!(review.mismatch_with_usage);
        assert_eq!(review.findings[0].line, Some(3));
        assert_eq!(review.findings[0].rule, "ai");
    }

    #[test]
    fn resposta_sem_json_e_erro() {
        assert!(parse_review("parece tudo certo", None).is_err());
        assert!(parse_review("{\"x\": 1}", None).is_err());
    }

    #[test]
    fn severidade_aceita_portugues() {
        assert_eq!(severity("Crítico"), Severity::Critical);
        assert_eq!(severity("médio"), Severity::Medium);
        assert_eq!(severity("???"), Severity::Low);
    }
}
