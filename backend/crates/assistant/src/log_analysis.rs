//! Análise de console pela IA: o operador manda o console de uma compilação,
//! de um deploy ou de um container e recebe, em vez de texto corrido, a lista
//! de erros, avisos, descontinuações e sugestões — cada um com a linha e o
//! trecho de onde saiu.
//!
//! O console passa por três etapas, todas puras e testadas aqui:
//!
//! 1. [`prepare`] troca credenciais pelo marcador
//!    ([`crate::redaction::mask_credentials`]), numera as linhas e, se o
//!    console não couber em [`MAX_SENT_CHARS`], manda as linhas que citam
//!    erro/aviso/descontinuação (com a vizinha de cada lado) e o final — e diz
//!    quantas ficaram de fora;
//! 2. a IA responde num JSON fixo (sem ferramentas: tudo o que ela precisa
//!    está no console);
//! 3. [`parse_answer`] lê esse JSON com tolerância (cerca de markdown, nome de
//!    campo trocado, linha como texto) e descarta linha que não existe. Se não
//!    houver JSON, o texto vira o resumo e o relatório sai como não
//!    estruturado — a tela não diz "nenhum problema" sem ter lido uma lista.

use std::{collections::BTreeSet, sync::LazyLock};

use chrono::Utc;
use regex::Regex;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::{
    drivers::{
        self,
        traits::{AiChatOptions, AiDriver, AiMessage},
    },
    dtos::LogAnalysisInput,
    proactive::runner::DriverFactory,
    redaction::mask_credentials,
    settings,
};
use netmonitor_core::shared::{
    errors::{AppError, AppResult},
    text::{collapse_whitespace, truncate_chars},
};

/// Maior console aceito, em bytes (o log de uma compilação para em 256 KiB).
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Caracteres do console que vão à IA (~8 mil tokens: cabe até num modelo
/// local de janela curta, com espaço para a resposta).
pub const MAX_SENT_CHARS: usize = 24_000;
/// Uma linha longa (JSON minificado, barra de progresso) é cortada aqui.
pub const MAX_LINE_CHARS: usize = 300;
/// Parte do orçamento reservada às linhas com erro/aviso; o resto vai ao final.
const NOTABLE_SHARE_PERCENT: usize = 60;
/// Itens no relatório, no máximo.
pub const MAX_FINDINGS: usize = 30;
const MAX_SOURCE_CHARS: usize = 120;
const MAX_TEXT_CHARS: usize = 600;
const MAX_SUMMARY_CHARS: usize = 2_000;
const ANSWER_MAX_TOKENS: u32 = 2_500;
/// Numeração + separador de cada linha enviada (`12345| `).
const LINE_OVERHEAD: usize = 8;

/// O que a tela mostra quando pedem a análise com a IA desligada.
pub const DISABLED_MESSAGE: &str =
    "A IA está desligada. Ligue em Configurações › Inteligência artificial para analisar o console.";

/// Linha que merece ir à IA mesmo quando o console não cabe inteiro.
static NOTABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(error|erro\b|err!|falh|fail|fatal|panic|exception|traceback|warn|aviso|deprecat|obsolet|descontinu|vulnerab|\bcve-\d|denied|negad|not found|exit code|exited with|timed? ?out)",
    )
    .expect("regex de linha notável")
});

const SYSTEM_PROMPT: &str = "Você analisa consoles de compilação, deploy e containers (Docker, BuildKit, \
Compose, npm, yarn, pnpm, cargo, pip, maven, gradle, go, dotnet). Aponte só o que está escrito no console: \
erros, avisos (warnings), descontinuações (deprecated/obsoleto) e sugestões que o próprio log indica. \
Cada linha vem numerada como `N| texto`; use esse N em \"line\" e copie em \"excerpt\" o trecho exato. \
Não invente linhas nem problemas: se não houver nada a apontar, devolva \"findings\" vazio. Credenciais \
aparecem como [credencial omitida] — isso não é um problema. Agrupe repetições do mesmo problema num item só \
(cite a primeira linha). Ordene do mais grave para o menos grave, no máximo 30 itens. Responda SOMENTE com \
um JSON, sem markdown e sem texto fora dele, neste formato: \
{\"summary\": \"uma ou duas frases em português sobre o resultado geral\", \"findings\": [{\"kind\": \
\"error|warning|deprecation|suggestion\", \"line\": 123, \"excerpt\": \"trecho exato da linha\", \"message\": \
\"o que significa, em português\", \"suggestion\": \"como resolver, em português\"}]}";

/// O console pronto para a IA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedLog {
    /// As linhas escolhidas, numeradas (`N| texto`), com os cortes marcados.
    pub text: String,
    pub total_lines: usize,
    pub sent_lines: usize,
    /// Alguma linha com conteúdo ficou de fora.
    pub truncated: bool,
    /// Alguma credencial foi trocada pelo marcador.
    pub masked: bool,
}

fn cost(line: &str) -> usize {
    line.chars().count() + LINE_OVERHEAD
}

/// Quais linhas vão quando o console não cabe: primeiro as notáveis com uma
/// vizinha de cada lado (até [`NOTABLE_SHARE_PERCENT`] do orçamento), depois
/// o final, de trás para frente, até encher.
fn select(lines: &[String]) -> BTreeSet<usize> {
    let mut chosen = BTreeSet::new();
    let mut used = 0;
    let notable_budget = MAX_SENT_CHARS * NOTABLE_SHARE_PERCENT / 100;
    let last = lines.len().saturating_sub(1);

    'notable: for index in (0..lines.len()).filter(|&i| NOTABLE.is_match(&lines[i])) {
        let first = index.saturating_sub(1);
        let neighbours = lines
            .iter()
            .enumerate()
            .skip(first)
            .take((index + 1).min(last) + 1 - first);
        for (near, line) in neighbours {
            if line.is_empty() || chosen.contains(&near) {
                continue;
            }
            let cost = cost(line);
            if used + cost > notable_budget {
                break 'notable;
            }
            used += cost;
            chosen.insert(near);
        }
    }
    for (index, line) in lines.iter().enumerate().rev() {
        if line.is_empty() || chosen.contains(&index) {
            continue;
        }
        let cost = cost(line);
        if used + cost > MAX_SENT_CHARS {
            break;
        }
        used += cost;
        chosen.insert(index);
    }
    chosen
}

fn omitted_marker(count: usize) -> String {
    if count == 1 {
        "… 1 linha omitida …\n".to_string()
    } else {
        format!("… {count} linhas omitidas …\n")
    }
}

/// Mascara, numera e, se preciso, recorta o console.
#[must_use]
pub fn prepare(content: &str) -> PreparedLog {
    let (masked_text, masked) = mask_credentials(content);
    let lines: Vec<String> = masked_text
        .lines()
        .map(|line| truncate_chars(line, MAX_LINE_CHARS))
        .collect();
    let filled = lines.iter().filter(|line| !line.is_empty()).count();
    let total_cost: usize = lines
        .iter()
        .filter(|line| !line.is_empty())
        .map(|line| cost(line))
        .sum();

    let chosen: BTreeSet<usize> = if total_cost <= MAX_SENT_CHARS {
        (0..lines.len()).filter(|&i| !lines[i].is_empty()).collect()
    } else {
        select(&lines)
    };

    let mut text = String::new();
    let mut next = 0;
    for &index in &chosen {
        let skipped = lines[next..index].iter().filter(|l| !l.is_empty()).count();
        if skipped > 0 {
            text.push_str(&omitted_marker(skipped));
        }
        text.push_str(&format!("{}| {}\n", index + 1, lines[index]));
        next = index + 1;
    }
    let skipped = lines[next.min(lines.len())..]
        .iter()
        .filter(|l| !l.is_empty())
        .count();
    if skipped > 0 {
        text.push_str(&omitted_marker(skipped));
    }

    PreparedLog {
        text,
        total_lines: lines.len(),
        sent_lines: chosen.len(),
        truncated: chosen.len() < filled,
        masked,
    }
}

/// O que um item do relatório é.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LogFindingKind {
    Error,
    Warning,
    Deprecation,
    Suggestion,
}

impl LogFindingKind {
    /// Lê o tipo com tolerância: o modelo escreve "warn", "Aviso",
    /// "deprecated", "critical"...
    fn parse(raw: &str) -> Self {
        let lower = raw.to_lowercase();
        if ["deprec", "obsolet", "descontinu"]
            .iter()
            .any(|word| lower.contains(word))
        {
            Self::Deprecation
        } else if ["warn", "aviso", "alerta"]
            .iter()
            .any(|word| lower.contains(word))
        {
            Self::Warning
        } else if ["err", "fatal", "falh", "fail", "crit"]
            .iter()
            .any(|word| lower.contains(word))
        {
            Self::Error
        } else {
            Self::Suggestion
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogFinding {
    pub kind: LogFindingKind,
    /// Linha do console (1 = a primeira); `None` quando a IA não citou uma
    /// que exista.
    pub line: Option<u32>,
    pub excerpt: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Resposta de `POST /api/ai/log-analysis`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LogAnalysisReport {
    pub source: String,
    pub summary: String,
    pub findings: Vec<LogFinding>,
    /// A IA respondeu no formato pedido. Falso: `summary` traz o texto dela
    /// e `findings` fica vazio sem querer dizer "nenhum problema".
    pub structured: bool,
    pub total_lines: u32,
    pub analyzed_lines: u32,
    pub truncated: bool,
    pub masked: bool,
    pub model: Option<String>,
    pub generated_at: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawReport {
    #[serde(default, alias = "resumo")]
    summary: Option<String>,
    #[serde(default, alias = "issues", alias = "problems", alias = "itens")]
    findings: Vec<RawFinding>,
}

#[derive(Debug, Default, Deserialize)]
struct RawFinding {
    #[serde(default, alias = "severity", alias = "type", alias = "tipo")]
    kind: Option<String>,
    #[serde(default, alias = "linha")]
    line: Option<Value>,
    #[serde(default, alias = "trecho", alias = "snippet")]
    excerpt: Option<String>,
    #[serde(default, alias = "mensagem", alias = "description")]
    message: Option<String>,
    #[serde(default, alias = "sugestao", alias = "fix", alias = "recommendation")]
    suggestion: Option<String>,
}

/// O objeto JSON da resposta: do primeiro `{` ao último `}` (descarta cerca
/// de markdown e conversa em volta).
fn json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

/// Número da linha citado, se existir no console ("12", 12, "12-14").
fn line_of(value: Option<&Value>, total_lines: usize) -> Option<u32> {
    let number = match value? {
        Value::Number(number) => number.as_u64()?,
        Value::String(text) => {
            let digits: String = text
                .trim()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()?
        }
        _ => return None,
    };
    let total = u64::try_from(total_lines).unwrap_or(u64::MAX);
    if (1..=total).contains(&number) {
        u32::try_from(number).ok()
    } else {
        None
    }
}

fn clean(text: Option<String>) -> Option<String> {
    text.map(|text| truncate_chars(&collapse_whitespace(&text), MAX_TEXT_CHARS))
        .filter(|text| !text.is_empty())
}

/// Resumo, itens e se a resposta veio no formato pedido.
#[must_use]
pub fn parse_answer(raw: &str, total_lines: usize) -> (String, Vec<LogFinding>, bool) {
    let Some(report) =
        json_object(raw).and_then(|json| serde_json::from_str::<RawReport>(json).ok())
    else {
        return (truncate_chars(raw, MAX_SUMMARY_CHARS), Vec::new(), false);
    };

    let mut findings: Vec<LogFinding> = report
        .findings
        .into_iter()
        .filter_map(|item| {
            let excerpt = clean(item.excerpt).unwrap_or_default();
            let message =
                clean(item.message).or_else(|| (!excerpt.is_empty()).then(|| excerpt.clone()))?;
            Some(LogFinding {
                kind: LogFindingKind::parse(item.kind.as_deref().unwrap_or_default()),
                line: line_of(item.line.as_ref(), total_lines),
                excerpt,
                message,
                suggestion: clean(item.suggestion),
            })
        })
        .collect();
    findings.sort_by_key(|finding| (finding.kind, finding.line.unwrap_or(u32::MAX)));
    findings.truncate(MAX_FINDINGS);

    let summary = report
        .summary
        .map(|text| truncate_chars(&text, MAX_SUMMARY_CHARS))
        .filter(|text| !text.is_empty())
        .unwrap_or_default();
    (summary, findings, true)
}

fn user_message(source: &str, prepared: &PreparedLog) -> String {
    let mut text = format!(
        "Console: {source}\nLinhas no console: {}; linhas enviadas: {}.\n",
        prepared.total_lines, prepared.sent_lines
    );
    if prepared.truncated {
        text.push_str(
            "O console não coube inteiro: vieram as linhas que citam erro, aviso ou \
             descontinuação (com a vizinha de cada lado) e o final.\n",
        );
    }
    text.push('\n');
    text.push_str(&prepared.text);
    text
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Manda o console preparado a `driver` e monta o relatório.
///
/// # Errors
///
/// Provedor indisponível ou resposta vazia.
pub async fn analyze_with(
    driver: &dyn AiDriver,
    source: &str,
    prepared: &PreparedLog,
) -> AppResult<LogAnalysisReport> {
    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: Some(SYSTEM_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
        },
        AiMessage {
            role: "user".to_string(),
            content: Some(user_message(source, prepared)),
            tool_calls: None,
            tool_call_id: None,
        },
    ];
    let answer = drivers::complete(
        driver,
        &messages,
        AiChatOptions {
            max_tokens: Some(ANSWER_MAX_TOKENS),
        },
    )
    .await?;
    if answer.is_empty() {
        return Err(AppError::service_unavailable(
            "O provedor de IA não devolveu a análise do console. Tente de novo.",
        ));
    }
    let (summary, findings, structured) = parse_answer(&answer, prepared.total_lines);
    Ok(LogAnalysisReport {
        source: source.to_string(),
        summary,
        findings,
        structured,
        total_lines: to_u32(prepared.total_lines),
        analyzed_lines: to_u32(prepared.sent_lines),
        truncated: prepared.truncated,
        masked: prepared.masked,
        model: driver.model().map(str::to_string),
        generated_at: Utc::now().to_rfc3339(),
    })
}

/// Valida o pedido, confere se a IA está ligada e analisa o console com o
/// provedor configurado.
///
/// # Errors
///
/// Console vazio ou grande demais (`Validation`), IA desligada
/// (`BusinessRule`), provedor não configurado ou indisponível.
pub async fn analyze(
    db: &DatabaseConnection,
    drivers: &dyn DriverFactory,
    input: LogAnalysisInput,
) -> AppResult<LogAnalysisReport> {
    if input.content.trim().is_empty() {
        return Err(AppError::validation(
            "O console está vazio: não há o que analisar.",
        ));
    }
    if input.content.len() > MAX_INPUT_BYTES {
        return Err(AppError::validation(
            "O console passa de 1 MiB. Copie o trecho que interessa e pergunte no chat da IA.",
        ));
    }
    let settings = settings::load(db).await?;
    if !settings.enabled {
        return Err(AppError::BusinessRule(DISABLED_MESSAGE.into()));
    }
    let source = truncate_chars(&collapse_whitespace(&input.source), MAX_SOURCE_CHARS);
    let source = if source.is_empty() {
        "Console".to_string()
    } else {
        source
    };
    let prepared = prepare(&input.content);
    let driver = drivers.create(&settings).await?;
    analyze_with(driver.as_ref(), &source, &prepared).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        drivers::traits::{AiChatChunk, AiChunkStream, AiTool},
        dtos::TestConnectionResponse,
    };
    use async_trait::async_trait;

    #[test]
    fn console_pequeno_vai_inteiro_e_numerado() {
        let prepared = prepare("Step 1/3\n\nnpm WARN deprecated left-pad@1.0.0\nDone");
        assert_eq!(prepared.total_lines, 4);
        assert_eq!(prepared.sent_lines, 3);
        assert!(!prepared.truncated);
        assert!(!prepared.masked);
        assert_eq!(
            prepared.text,
            "1| Step 1/3\n3| npm WARN deprecated left-pad@1.0.0\n4| Done\n"
        );
    }

    #[test]
    fn console_grande_prioriza_erros_e_o_final() {
        let mut lines: Vec<String> = (1..=3_000)
            .map(|n| format!("#{n} passo comum de compilação"))
            .collect();
        lines[99] = "npm ERR! code ERESOLVE".to_string();
        lines[1_499] = "WARNING: package xyz is deprecated".to_string();
        let prepared = prepare(&lines.join("\n"));

        assert!(prepared.truncated);
        assert_eq!(prepared.total_lines, 3_000);
        assert!(prepared.sent_lines < 3_000);
        assert!(prepared.text.chars().count() <= MAX_SENT_CHARS + 2_000);
        assert!(prepared.text.contains("100| npm ERR! code ERESOLVE"));
        assert!(
            prepared.text.contains("99| #99"),
            "vizinha de cima vai junto"
        );
        assert!(prepared
            .text
            .contains("1500| WARNING: package xyz is deprecated"));
        assert!(prepared.text.contains("3000| #3000"), "o final vai");
        assert!(prepared.text.contains("linhas omitidas"));
        assert!(
            !prepared.text.contains("2| #2 "),
            "o miolo comum fica de fora"
        );
    }

    #[test]
    fn credenciais_saem_antes_de_ir_a_ia() {
        let prepared = prepare("clone https://user:s3nh4@git.example.com/repo\nTOKEN=abc123");
        assert!(prepared.masked);
        assert!(!prepared.text.contains("s3nh4"));
        assert!(!prepared.text.contains("abc123"));
    }

    #[test]
    fn linha_longa_e_cortada() {
        let prepared = prepare(&"x".repeat(5_000));
        assert!(prepared.text.chars().count() < MAX_LINE_CHARS + 20);
    }

    #[test]
    fn le_json_com_cerca_de_markdown_e_ordena_por_gravidade() {
        let raw = r#"```json
{"summary": "Dois problemas.", "findings": [
  {"kind": "deprecated", "line": "7", "excerpt": "npm WARN deprecated", "message": "Pacote obsoleto"},
  {"severity": "ERROR", "line": 3, "excerpt": "npm ERR!", "message": "Falhou", "suggestion": "Rode npm ci"},
  {"kind": "warning", "line": 999, "message": "Linha inventada"},
  {"kind": "error"}
]}
```"#;
        let (summary, findings, structured) = parse_answer(raw, 10);
        assert!(structured);
        assert_eq!(summary, "Dois problemas.");
        assert_eq!(
            findings.len(),
            3,
            "item sem mensagem nem trecho é descartado"
        );
        assert_eq!(findings[0].kind, LogFindingKind::Error);
        assert_eq!(findings[0].line, Some(3));
        assert_eq!(findings[0].suggestion.as_deref(), Some("Rode npm ci"));
        assert_eq!(findings[1].kind, LogFindingKind::Warning);
        assert_eq!(findings[1].line, None, "linha que não existe no console");
        assert_eq!(findings[2].kind, LogFindingKind::Deprecation);
        assert_eq!(findings[2].line, Some(7));
    }

    #[test]
    fn resposta_sem_json_vira_resumo_nao_estruturado() {
        let (summary, findings, structured) = parse_answer("Não encontrei problemas.", 3);
        assert!(!structured);
        assert!(findings.is_empty());
        assert_eq!(summary, "Não encontrei problemas.");
    }

    #[test]
    fn lista_vazia_e_estruturada() {
        let (_, findings, structured) =
            parse_answer(r#"{"summary": "Tudo certo", "findings": []}"#, 3);
        assert!(structured);
        assert!(findings.is_empty());
    }

    struct Fixo(&'static str);

    #[async_trait]
    impl AiDriver for Fixo {
        fn id(&self) -> &'static str {
            "fixo"
        }
        fn display_name(&self) -> &'static str {
            "Fixo"
        }
        fn model(&self) -> Option<&str> {
            Some("modelo-teste")
        }
        async fn chat_stream(
            &self,
            messages: &[AiMessage],
            tools: &[AiTool],
            _options: AiChatOptions,
        ) -> AppResult<AiChunkStream> {
            assert!(tools.is_empty(), "a análise não usa ferramentas");
            let user = messages[1].content.as_deref().unwrap_or_default();
            assert!(user.contains("Console: Compilação #7"));
            assert!(user.contains("2| npm ERR! boom"));
            let chunks = self
                .0
                .split_inclusive(' ')
                .map(|part| {
                    Ok(AiChatChunk {
                        text_delta: Some(part.to_string()),
                        ..AiChatChunk::default()
                    })
                })
                .collect::<Vec<_>>();
            Ok(Box::pin(futures::stream::iter(chunks)))
        }
        async fn test_connection(&self) -> AppResult<TestConnectionResponse> {
            unreachable!("não usado no teste")
        }
    }

    #[tokio::test]
    async fn analisa_com_o_driver_e_monta_o_relatorio() {
        let prepared = prepare("Step 1\nnpm ERR! boom");
        let driver = Fixo(
            r#"{"summary": "A instalação falhou.", "findings": [{"kind": "error", "line": 2, "excerpt": "npm ERR! boom", "message": "O npm falhou"}]}"#,
        );
        let report = analyze_with(&driver, "Compilação #7", &prepared)
            .await
            .unwrap();
        assert!(report.structured);
        assert_eq!(report.summary, "A instalação falhou.");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].line, Some(2));
        assert_eq!(report.total_lines, 2);
        assert_eq!(report.analyzed_lines, 2);
        assert_eq!(report.model.as_deref(), Some("modelo-teste"));
    }

    #[tokio::test]
    async fn resposta_vazia_e_erro() {
        let prepared = prepare("Step 1\nnpm ERR! boom");
        let err = analyze_with(&Fixo("   "), "Compilação #7", &prepared)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("não devolveu"));
    }
}
