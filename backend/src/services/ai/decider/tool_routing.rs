//! Primeira decisão do Decisor: quais grupos de ferramentas a pergunta pede.
//!
//! Uma pergunta sim/não por grupo, todas na mesma passada — uma pergunta
//! costuma pedir mais de um grupo, então uma escolha única não serviria.
//!
//! Com o Decisor no ar, é ele quem escolhe o que vai ao chat (além do núcleo
//! e do que a pessoa marcou); errar para menos custa uma rodada de
//! `load_tools`. Sem ele — desligado, sem modelo, carregando ou falhando —
//! [`route`] devolve `None` e o chat recebe o catálogo inteiro: nada fica de
//! fora por falta de quem escolha.

use std::collections::BTreeMap;

use super::{
    config::{AiDeciderSettings, DeciderFeature},
    decision::{Decision, Outcome},
    runtime::{DeciderRuntime, Lane},
    schema::{Answer, Question},
};
use crate::services::{
    ai::harness::tools::{ToolGroup, ToolGroups},
    shared::errors::AppResult,
};

/// Quão provável é cada grupo ser necessário, na ordem do catálogo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoutingScores(pub Vec<(ToolGroup, f64)>);

impl RoutingScores {
    /// Os grupos com probabilidade no limiar ou acima dele.
    #[must_use]
    pub fn selected(&self, threshold: f64) -> ToolGroups {
        self.0
            .iter()
            .filter(|(_, probability)| *probability >= threshold)
            .map(|(group, _)| *group)
            .collect()
    }
}

pub struct ToolRouting<'a> {
    /// Só os grupos que a política da sessão libera: perguntar pelos outros
    /// é latência sem uso.
    pub groups: &'a [ToolGroup],
}

impl Decision for ToolRouting<'_> {
    type Output = RoutingScores;

    fn questions(&self) -> BTreeMap<String, Question> {
        self.groups
            .iter()
            .map(|group| {
                (
                    group.id().to_string(),
                    Question::Noul {
                        instructions: group.intent().to_string(),
                    },
                )
            })
            .collect()
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> RoutingScores {
        RoutingScores(
            self.groups
                .iter()
                .map(|group| {
                    let probability = answers.get(group.id()).and_then(Answer::yes).unwrap_or(0.0);
                    (*group, probability)
                })
                .collect(),
        )
    }
}

/// Pontua os grupos para a pergunta, pela fila dada do runtime.
///
/// # Errors
///
/// Decisor fora do ar, modelo frio no chat ou resposta inválida.
pub async fn score(
    runtime: &DeciderRuntime,
    settings: &AiDeciderSettings,
    lane: Lane,
    question: &str,
    groups: &[ToolGroup],
) -> AppResult<Outcome<RoutingScores>> {
    runtime
        .try_decide_with(settings, lane, question, &ToolRouting { groups })
        .await
}

/// Os grupos que o Decisor escolhe para a pergunta do chat.
///
/// `None` quando não há Decisor para escolher — desligado, sem modelo, ainda
/// carregando ou com falha nesta pergunta: o chat manda então o catálogo
/// inteiro, e nunca espera nem quebra por causa dele. `Some` vazio é escolha:
/// a pergunta só precisa do núcleo.
pub async fn route(
    runtime: &DeciderRuntime,
    settings: &AiDeciderSettings,
    question: &str,
    groups: &[ToolGroup],
) -> Option<ToolGroups> {
    if !settings.allows(DeciderFeature::ChatTools) {
        return None;
    }
    if groups.is_empty() || question.trim().is_empty() {
        return Some(ToolGroups::new());
    }
    match score(runtime, settings, Lane::Interactive, question, groups).await {
        Ok(outcome) => {
            let selected = outcome.value.selected(settings.threshold());
            tracing::debug!(
                model = %outcome.model,
                latency_ms = outcome.latency_ms,
                groups = ?selected.ids(),
                "decisor: grupos de ferramentas decididos"
            );
            Some(selected)
        }
        Err(error) => {
            tracing::debug!(%error, "decisor sem resposta nesta pergunta; o chat recebe todas as ferramentas");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUPS: [ToolGroup; 3] = [ToolGroup::History, ToolGroup::Docker, ToolGroup::Charts];

    #[test]
    fn pergunta_uma_afirmacao_por_grupo() {
        let questions = ToolRouting { groups: &GROUPS }.questions();
        assert_eq!(questions.len(), 3);
        assert_eq!(
            questions["docker"],
            Question::Noul {
                instructions: ToolGroup::Docker.intent().to_string()
            }
        );
    }

    #[test]
    fn seleciona_no_limiar_e_ignora_resposta_ausente_ou_estranha() {
        let answers = BTreeMap::from([
            ("history".to_string(), Answer::Noul { noul: 0.6 }),
            ("docker".to_string(), Answer::Noul { noul: 0.59 }),
            ("charts".to_string(), Answer::Other),
        ]);
        let scores = ToolRouting { groups: &GROUPS }.interpret(&answers);
        assert_eq!(
            scores.0,
            vec![
                (ToolGroup::History, 0.6),
                (ToolGroup::Docker, 0.59),
                (ToolGroup::Charts, 0.0),
            ]
        );
        assert_eq!(scores.selected(0.6), ToolGroups::from([ToolGroup::History]));
    }

    #[tokio::test]
    async fn desligado_nao_consulta_nada_e_o_chat_recebe_tudo() {
        let settings = AiDeciderSettings {
            // Endereço que não responde: se consultasse, o teste demoraria/falharia.
            base_url: "http://127.0.0.1:9".into(),
            ..AiDeciderSettings::default()
        };
        assert!(route(
            &DeciderRuntime::default(),
            &settings,
            "por que caiu?",
            &GROUPS
        )
        .await
        .is_none());
    }

    #[tokio::test]
    async fn servidor_fora_do_ar_conta_como_sem_decisor() {
        let settings = AiDeciderSettings {
            enabled: true,
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 500,
            ..AiDeciderSettings::default()
        };
        assert!(route(
            &DeciderRuntime::default(),
            &settings,
            "por que caiu?",
            &GROUPS
        )
        .await
        .is_none());
    }
}
