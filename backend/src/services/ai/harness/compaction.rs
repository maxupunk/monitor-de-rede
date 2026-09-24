//! Compactação do contexto: quando a conversa se aproxima do limite da janela
//! do modelo, as mensagens antigas viram um resumo.
//!
//! # As margens
//!
//! Três limites, sobre a janela `W` do modelo, com `R` tokens reservados para
//! a resposta:
//!
//! | limite | valor | para quê |
//! |---|---|---|
//! | gatilho | `min(80% W, 90% W − R)` | compactar antes de a resposta ficar sem espaço |
//! | teto | `90% W − R` | nunca mandar mais que isso: 10% absorvem o erro da estimativa |
//! | meta | `50% W` | depois de compactar, sobra meia janela para a conversa crescer |
//!
//! A estimativa de tokens é conservadora (3 caracteres por token — texto em
//! português e JSON de ferramentas ficam entre 3 e 4): errar para mais
//! compacta um pouco antes; errar para menos faria o provedor cortar a
//! conversa sozinho, ou recusar a requisição.
//!
//! Dentro de uma mesma resposta, várias rodadas de ferramentas também enchem
//! a janela. Aí não há o que resumir: os resultados de ferramenta das rodadas
//! anteriores, que a IA já leu, dão lugar a um aviso ([`prune_tool_results`]).

use futures::StreamExt;

use crate::{
    dtos::ai::ChatMessageInput,
    services::{
        ai::drivers::traits::{AiChatOptions, AiDriver, AiMessage},
        shared::errors::{AppError, AppResult},
    },
};

/// Uso da janela a partir do qual a conversa é compactada.
pub const COMPACT_AT: f64 = 0.80;
/// Teto de uso, já contando a resposta.
pub const HARD_LIMIT: f64 = 0.90;
/// Uso-alvo depois de compactar.
pub const TARGET_AFTER: f64 = 0.50;

/// Caracteres por token na estimativa — conservadora de propósito.
const CHARS_PER_TOKEN: u64 = 3;
/// Custo fixo de cada mensagem no protocolo (papel, separadores).
const MESSAGE_OVERHEAD: u64 = 4;
/// Resposta reservada quando o estilo não limita a saída.
const DEFAULT_OUTPUT_RESERVE: u64 = 4_096;
/// Tamanho máximo do resumo gerado.
const SUMMARY_MAX_TOKENS: u32 = 700;
/// Aviso que substitui um resultado de ferramenta podado.
pub const PRUNED_NOTICE: &str =
    "[resultado omitido para caber na janela de contexto; chame a ferramenta de novo se precisar]";

/// Tokens estimados de um texto.
#[must_use]
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64).div_ceil(CHARS_PER_TOKEN)
}

/// Tokens estimados de mensagens já no formato do provedor.
#[must_use]
pub fn estimate_messages(messages: &[AiMessage]) -> u64 {
    messages
        .iter()
        .map(|message| {
            let calls = message.tool_calls.as_ref().map_or(0, |calls| {
                calls
                    .iter()
                    .map(|call| estimate_tokens(&call.name) + estimate_tokens(&call.arguments))
                    .sum()
            });
            MESSAGE_OVERHEAD + estimate_tokens(message.content.as_deref().unwrap_or("")) + calls
        })
        .sum()
}

/// Tokens estimados do histórico vindo da tela.
#[must_use]
pub fn estimate_history(history: &[ChatMessageInput]) -> u64 {
    history
        .iter()
        .map(|message| MESSAGE_OVERHEAD + estimate_tokens(&message.content))
        .sum()
}

fn fraction(window: u64, ratio: f64) -> u64 {
    // Janelas cabem com folga em f64 (53 bits); o arredondamento é para baixo.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let value = (window as f64 * ratio) as u64;
    value
}

/// Os limites de uso de uma janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBudget {
    pub window: u64,
    pub reserved_output: u64,
}

impl ContextBudget {
    /// `max_output` é o teto de saída do estilo de resposta; a reserva nunca
    /// passa de um quarto da janela (modelos pequenos do Ollama).
    #[must_use]
    pub fn new(window: u64, max_output: Option<u32>) -> Self {
        let reserved = max_output.map_or(DEFAULT_OUTPUT_RESERVE, u64::from);
        Self {
            window,
            reserved_output: reserved.min(window / 4),
        }
    }

    /// Entrada máxima: nunca acima dela.
    #[must_use]
    pub fn ceiling(&self) -> u64 {
        fraction(self.window, HARD_LIMIT).saturating_sub(self.reserved_output)
    }

    /// Entrada a partir da qual a conversa é compactada.
    #[must_use]
    pub fn trigger(&self) -> u64 {
        fraction(self.window, COMPACT_AT).min(self.ceiling())
    }

    /// Entrada desejada depois de compactar.
    #[must_use]
    pub fn target(&self) -> u64 {
        fraction(self.window, TARGET_AFTER).min(self.ceiling())
    }

    #[must_use]
    pub fn needs_compaction(&self, input_tokens: u64) -> bool {
        input_tokens >= self.trigger()
    }
}

/// Quantas mensagens do começo do histórico viram resumo.
///
/// Ficam as mais recentes que cabem na meta, descontado o que é fixo (prompt
/// de sistema, ferramentas) e o espaço do próprio resumo. O trecho mantido
/// começa sempre por uma pergunta do usuário — uma resposta sem a pergunta
/// confunde o modelo — e a pergunta atual nunca é resumida. `force` compacta
/// mesmo cabendo: é o pedido explícito da tela.
#[must_use]
pub fn fold_count(
    history: &[ChatMessageInput],
    fixed_tokens: u64,
    budget: &ContextBudget,
    force: bool,
) -> usize {
    if history.len() < 2 {
        return 0;
    }
    let room = budget
        .target()
        .saturating_sub(fixed_tokens)
        .saturating_sub(u64::from(SUMMARY_MAX_TOKENS));
    let mut kept_tokens = 0;
    let mut keep_from = history.len() - 1;
    kept_tokens += MESSAGE_OVERHEAD + estimate_tokens(&history[keep_from].content);
    while keep_from > 0 {
        let cost = MESSAGE_OVERHEAD + estimate_tokens(&history[keep_from - 1].content);
        if kept_tokens + cost > room {
            break;
        }
        kept_tokens += cost;
        keep_from -= 1;
    }
    // Compactação pedida: o resumo precisa cobrir ao menos a troca anterior.
    if force && keep_from == 0 {
        keep_from = history.len() - 1;
    }
    // O trecho mantido começa numa pergunta do usuário.
    while keep_from < history.len() - 1 && history[keep_from].role != "user" {
        keep_from += 1;
    }
    keep_from
}

/// Poda os resultados de ferramenta mais antigos até a conversa caber no
/// teto. As mensagens a partir de `protect_from` (a rodada atual) ficam
/// intactas. Devolve se algo foi podado.
pub fn prune_tool_results(
    conversation: &mut [AiMessage],
    extra_tokens: u64,
    budget: &ContextBudget,
    protect_from: usize,
) -> bool {
    let mut total = estimate_messages(conversation) + extra_tokens;
    let mut pruned = false;
    for message in conversation.iter_mut().take(protect_from) {
        if total < budget.ceiling() {
            break;
        }
        if message.role != "tool" || message.content.as_deref() == Some(PRUNED_NOTICE) {
            continue;
        }
        let before = estimate_tokens(message.content.as_deref().unwrap_or(""));
        message.content = Some(PRUNED_NOTICE.to_string());
        total = total.saturating_sub(before.saturating_sub(estimate_tokens(PRUNED_NOTICE)));
        pruned = true;
    }
    pruned
}

const SUMMARY_PROMPT: &str = "Você resume conversas entre um operador de rede e o assistente do NetMonitor. \
O resumo substitui as mensagens na memória do assistente, então preserve o que ele precisa para continuar: \
equipamentos, IPs, ids de alertas/monitores/regras, problemas encontrados e causas, números medidos, \
decisões e ações já executadas ou recusadas, e o que ficou pendente. \
Em tópicos curtos, no máximo 250 palavras, sem introdução.";

/// Transcrição do trecho a resumir, cortada para caber na chamada de resumo.
fn transcript(previous: Option<&str>, folded: &[ChatMessageInput], max_chars: usize) -> String {
    let mut text = String::new();
    if let Some(previous) = previous
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
    {
        text.push_str("Resumo anterior:\n");
        text.push_str(previous);
        text.push_str("\n\nMensagens seguintes:\n");
    }
    for message in folded {
        let speaker = if message.role == "user" {
            "Operador"
        } else {
            "Assistente"
        };
        text.push_str(speaker);
        text.push_str(": ");
        text.push_str(message.content.trim());
        text.push('\n');
    }
    if text.chars().count() <= max_chars {
        return text;
    }
    // Mantém o fim: o mais recente pesa mais para continuar a conversa.
    let skip = text.chars().count() - max_chars;
    format!("[…]{}", text.chars().skip(skip).collect::<String>())
}

/// Gera o resumo do trecho `folded`, incorporando o resumo anterior.
///
/// # Errors
///
/// Provedor indisponível ou resposta vazia — quem chama decide o plano B.
pub async fn summarize(
    driver: &dyn AiDriver,
    budget: &ContextBudget,
    previous: Option<&str>,
    folded: &[ChatMessageInput],
) -> AppResult<String> {
    // A chamada de resumo também precisa caber na janela.
    let max_chars = usize::try_from(budget.target().saturating_mul(CHARS_PER_TOKEN).max(4_000))
        .unwrap_or(usize::MAX);
    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: Some(SUMMARY_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
        },
        AiMessage {
            role: "user".to_string(),
            content: Some(transcript(previous, folded, max_chars)),
            tool_calls: None,
            tool_call_id: None,
        },
    ];
    let mut stream = driver
        .chat_stream(
            &messages,
            &[],
            AiChatOptions {
                max_tokens: Some(SUMMARY_MAX_TOKENS),
            },
        )
        .await?;
    let mut summary = String::new();
    while let Some(chunk) = stream.next().await {
        if let Some(delta) = chunk?.text_delta {
            summary.push_str(&delta);
        }
    }
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(AppError::service_unavailable(
            "O provedor não devolveu o resumo da conversa",
        ));
    }
    Ok(summary)
}

/// Plano B quando o provedor não resume: o resumo anterior segue, com o aviso
/// de que houve corte — melhor que estourar a janela.
#[must_use]
pub fn fallback_summary(previous: Option<&str>, folded: usize) -> String {
    let notice = format!(
        "({folded} mensagens antigas foram removidas da memória sem resumo para caber na janela de contexto.)"
    );
    match previous
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
    {
        Some(previous) => format!("{previous}\n{notice}"),
        None => notice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessageInput {
        ChatMessageInput {
            role: role.into(),
            content: content.into(),
        }
    }

    fn tool(content: &str) -> AiMessage {
        AiMessage {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some("c".into()),
        }
    }

    #[test]
    fn margens_da_janela() {
        let grande = ContextBudget::new(128_000, Some(1024));
        assert_eq!(grande.reserved_output, 1024);
        assert_eq!(grande.ceiling(), 115_200 - 1024);
        assert_eq!(
            grande.trigger(),
            102_400,
            "80% quando sobra espaço para a resposta"
        );
        assert_eq!(grande.target(), 64_000);

        let pequena = ContextBudget::new(4_096, None);
        assert_eq!(
            pequena.reserved_output, 1_024,
            "reserva limitada a 1/4 da janela"
        );
        assert_eq!(pequena.ceiling(), 3_686 - 1_024);
        assert_eq!(
            pequena.trigger(),
            pequena.ceiling(),
            "numa janela pequena o teto chega antes dos 80%"
        );
        assert!(pequena.needs_compaction(2_700));
        assert!(!pequena.needs_compaction(2_000));
    }

    #[test]
    fn estimativa_conservadora() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abc"), 1);
        assert_eq!(estimate_tokens("abcd"), 2);
        assert_eq!(
            estimate_tokens(&"ç".repeat(300)),
            100,
            "conta caracteres, não bytes"
        );
    }

    #[test]
    fn compacta_o_comeco_e_mantem_as_trocas_recentes_a_partir_de_uma_pergunta() {
        let longo = "x".repeat(3_000); // ~1000 tokens
        let historico = vec![
            msg("user", &longo),
            msg("assistant", &longo),
            msg("user", &longo),
            msg("assistant", &longo),
            msg("user", "e agora?"),
        ];
        // meta 5000 − fixo 1000 − resumo 700 = 3300: cabem a pergunta atual e mais três.
        let orcamento = ContextBudget::new(10_000, Some(500));
        let dobrar = fold_count(&historico, 1_000, &orcamento, false);
        assert_eq!(dobrar, 2);
        assert_eq!(historico[dobrar].role, "user");

        // Sem espaço nenhum: só a pergunta atual fica.
        assert_eq!(fold_count(&historico, 50_000, &orcamento, false), 4);
        assert_eq!(fold_count(&historico[..1], 0, &orcamento, false), 0);
    }

    #[test]
    fn compactacao_pedida_resume_mesmo_cabendo() {
        let historico = vec![
            msg("user", "oi"),
            msg("assistant", "olá"),
            msg("user", "ping no gateway"),
        ];
        let orcamento = ContextBudget::new(128_000, None);
        assert_eq!(fold_count(&historico, 0, &orcamento, false), 0);
        assert_eq!(fold_count(&historico, 0, &orcamento, true), 2);
    }

    #[test]
    fn poda_resultados_antigos_e_preserva_a_rodada_atual() {
        let grande = "r".repeat(6_000); // ~2000 tokens
        let mut conversa = vec![
            AiMessage {
                role: "system".into(),
                content: Some("s".into()),
                tool_calls: None,
                tool_call_id: None,
            },
            tool(&grande),
            tool(&grande),
            tool(&grande),
        ];
        let orcamento = ContextBudget::new(6_000, Some(500));
        assert!(prune_tool_results(&mut conversa, 0, &orcamento, 3));
        assert_eq!(conversa[1].content.as_deref(), Some(PRUNED_NOTICE));
        assert_eq!(
            conversa[3].content.as_deref(),
            Some(grande.as_str()),
            "a rodada atual fica"
        );
        assert!(estimate_messages(&conversa) < orcamento.ceiling());
        assert!(
            !prune_tool_results(&mut conversa, 0, &orcamento, 3),
            "já cabe"
        );
    }

    #[test]
    fn transcricao_mantem_o_fim_quando_passa_do_limite() {
        let trecho = vec![msg("user", &"a".repeat(100)), msg("assistant", "fim")];
        let texto = transcript(Some("antes"), &trecho, 30);
        assert!(texto.starts_with("[…]"));
        assert!(texto.ends_with("Assistente: fim\n"));
        assert!(transcript(None, &trecho, 10_000).starts_with("Operador: "));
    }

    #[test]
    fn plano_b_preserva_o_resumo_anterior() {
        assert!(fallback_summary(Some("rede A com perda"), 6).starts_with("rede A com perda\n"));
        assert!(fallback_summary(None, 6).contains("6 mensagens"));
    }
}
