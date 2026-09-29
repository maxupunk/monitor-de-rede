//! Uma decisão do Laya: as perguntas que ela faz e como ler as respostas.
//!
//! Cada decisão nova (roteamento de ferramentas hoje; estilo de resposta,
//! urgência de alerta, triagem do proativo amanhã) implementa [`Decision`] e
//! reaproveita o cliente, o tempo limite e o tratamento de erro — nada disso
//! muda quando uma decisão entra.

use std::collections::BTreeMap;

use super::{
    client::LayaClient,
    schema::{Answer, Question},
};
use crate::services::shared::errors::AppResult;

/// O Laya lê até ~1.024 tokens; o começo da pergunta é o que decide.
const MAX_STATE_CHARS: usize = 3_000;

pub trait Decision {
    type Output;

    /// As perguntas, pela chave que volta em `answers`.
    fn questions(&self) -> BTreeMap<String, Question>;

    /// Lê as respostas. Resposta ausente ou de outro tipo conta como "não sei".
    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> Self::Output;
}

/// O resultado de uma decisão e de onde ele veio.
#[derive(Debug, Clone)]
pub struct Outcome<T> {
    pub value: T,
    /// O modelo que respondeu (`laya:en`, `laya:multilingual`…).
    pub model: String,
    pub latency_ms: f64,
}

/// Corta o estado no limite de caracteres, sem partir um caractere.
fn clip(state: &str) -> &str {
    state
        .char_indices()
        .nth(MAX_STATE_CHARS)
        .map_or(state, |(end, _)| &state[..end])
}

pub async fn run<D: Decision>(
    client: &LayaClient<'_>,
    state: &str,
    decision: &D,
) -> AppResult<Outcome<D::Output>> {
    let decided = client
        .decide(clip(state.trim()), &decision.questions())
        .await?;
    Ok(Outcome {
        value: decision.interpret(&decided.response.answers),
        model: decided.response.model,
        latency_ms: decided.latency_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corta_o_estado_sem_partir_caractere() {
        let long = "ç".repeat(MAX_STATE_CHARS + 10);
        assert_eq!(clip(&long).chars().count(), MAX_STATE_CHARS);
        assert_eq!(clip("curta"), "curta");
    }
}
