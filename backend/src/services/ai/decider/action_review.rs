//! Segunda opinião do Decisor antes de o modo automático rodar uma ação.
//!
//! A regra de cada ferramenta ([`RiskAssessment`]) não enxerga o contexto:
//! reiniciar um container é risco médio, mas reiniciar o banco no meio de um
//! incidente, sem o usuário ter pedido, não é. O Decisor lê o pedido e a ação
//! proposta e diz se rodar agora é perigoso.
//!
//! Ele **só sobe** o risco: um "perigoso" vira pedido de confirmação; um
//! "seguro" não libera o que a regra já mandou perguntar. Errar para mais
//! custa um clique; errar para menos pode derrubar a rede. Fora do ar, frio
//! ou desligado, vale a regra sozinha.

use std::collections::BTreeMap;

use super::{
    config::{AiDeciderSettings, DeciderFeature},
    decision::Decision,
    runtime::{DeciderRuntime, Lane},
    schema::{Answer, Question},
};
use crate::services::ai::harness::tools::{AiToolRisk, RiskAssessment};

const DANGEROUS: &str = "dangerous";

/// Motivo mostrado ao usuário quando o Decisor pede a confirmação.
pub const ESCALATION_REASON: &str =
    "O Decisor avaliou que rodar isto agora, neste contexto, pode causar impacto";

/// O que o Decisor lê: o pedido do usuário e a ação que a IA quer rodar.
pub struct ActionReview<'a> {
    pub request: &'a str,
    pub action: &'a str,
    pub risk: &'a RiskAssessment,
}

impl ActionReview<'_> {
    /// Só a ação que o modo automático rodaria e que tem o que perder:
    /// risco baixo não justifica a latência, e o alto já pergunta.
    #[must_use]
    pub fn worth_reviewing(risk: &RiskAssessment) -> bool {
        risk.level == AiToolRisk::Medium
    }

    fn state(&self) -> String {
        format!(
            "User request: {}\nProposed action: {}\nRule-based risk: {} ({})",
            self.request.trim(),
            self.action.trim(),
            self.risk.level.id(),
            self.risk.reason
        )
    }
}

impl Decision for ActionReview<'_> {
    /// Probabilidade (0–1) de rodar agora ser perigoso.
    type Output = f64;

    fn questions(&self) -> BTreeMap<String, Question> {
        BTreeMap::from([(
            DANGEROUS.to_string(),
            Question::Noul {
                instructions: "Running the proposed action right now, without asking the user, \
could cause an outage, lose data or hide a real problem from the operators."
                    .to_string(),
            },
        )])
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> f64 {
        answers.get(DANGEROUS).and_then(Answer::yes).unwrap_or(0.0)
    }
}

/// `true` quando o Decisor acha perigoso rodar a ação sem perguntar. Desligado,
/// fora do ar ou com o modelo frio, `false`: o chat não espera por ele e
/// vale a regra da ferramenta.
pub async fn flags_danger(
    runtime: &DeciderRuntime,
    settings: &AiDeciderSettings,
    review: &ActionReview<'_>,
) -> bool {
    if !settings.allows(DeciderFeature::ActionReview) || !ActionReview::worth_reviewing(review.risk)
    {
        return false;
    }
    match runtime
        .try_decide_with(settings, Lane::Interactive, &review.state(), review)
        .await
    {
        Ok(outcome) => {
            tracing::debug!(
                model = %outcome.model,
                latency_ms = outcome.latency_ms,
                dangerous = outcome.value,
                "decisor: ação revisada"
            );
            outcome.value >= settings.threshold()
        }
        Err(error) => {
            tracing::debug!(%error, "decisor sem resposta na revisão da ação; vale a regra da ferramenta");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_revisa_o_risco_medio() {
        assert!(ActionReview::worth_reviewing(&RiskAssessment::medium("x")));
        assert!(!ActionReview::worth_reviewing(&RiskAssessment::low("x")));
        assert!(!ActionReview::worth_reviewing(&RiskAssessment::high("x")));
    }

    #[test]
    fn estado_leva_pedido_acao_e_risco() {
        let risk = RiskAssessment::medium("O serviço fica fora por alguns segundos");
        let review = ActionReview {
            request: " reinicie o banco ",
            action: "Reiniciar o container postgres (central)",
            risk: &risk,
        };
        let state = review.state();
        assert!(state.starts_with("User request: reinicie o banco\n"));
        assert!(state.contains("Proposed action: Reiniciar o container postgres (central)"));
        assert!(state.contains("Rule-based risk: medium"));
    }

    #[test]
    fn resposta_ausente_conta_como_seguro() {
        let risk = RiskAssessment::medium("x");
        let review = ActionReview {
            request: "",
            action: "",
            risk: &risk,
        };
        assert!(review.interpret(&BTreeMap::new()).abs() < f64::EPSILON);
        let answers = BTreeMap::from([(DANGEROUS.to_string(), Answer::Noul { noul: 0.8 })]);
        assert!((review.interpret(&answers) - 0.8).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn desligado_nao_consulta_nada() {
        let settings = AiDeciderSettings {
            base_url: "http://127.0.0.1:9".into(),
            ..AiDeciderSettings::default()
        };
        let risk = RiskAssessment::medium("x");
        let review = ActionReview {
            request: "reinicie",
            action: "Reiniciar",
            risk: &risk,
        };
        assert!(!flags_danger(&DeciderRuntime::default(), &settings, &review).await);
    }
}
