//! Rodar ou perguntar: o portão de cada chamada de ferramenta no chat.
//!
//! Duas camadas, nesta ordem:
//!
//! 1. **A regra da ferramenta** ([`ToolRegistry::assess`]): o modo
//!    configurado e o risco da chamada. "Pedir permissão" sempre pergunta;
//!    "automático" roda o risco baixo e o médio e pergunta o alto.
//! 2. **O Decisor** ([`action_review`]): no automático, lê o pedido do usuário e
//!    a ação de risco médio e pode mandá-la de volta para confirmação. Só
//!    sobe o risco — nunca libera o que a regra mandou perguntar.
//!
//! O acesso a equipamento não passa por aqui quando o "Aceitar
//! automaticamente" da conversa está ligado (ADR 012): esse atalho é do
//! laço do agente.

use loco_rs::prelude::AppContext;

use super::tools::{
    Clearance, RiskAssessment, ToolArgs, ToolKind, ToolOutput, ToolRegistry, Verdict,
};
use crate::services::{
    ai::decider::{
        action_review::{self, ActionReview, ESCALATION_REASON},
        config::AiDeciderSettings,
        runtime::DeciderRuntime,
    },
    shared::errors::{AppError, AppResult},
};

/// O Decisor e o pedido do usuário, para revisar as ações do automático.
pub struct ActionReviewer {
    pub runtime: DeciderRuntime,
    pub settings: AiDeciderSettings,
    /// A última pergunta do usuário: o contexto da ação.
    pub request: String,
}

/// O que fazer com a chamada.
#[derive(Debug)]
pub enum GateDecision {
    Run(RunPlan),
    /// Volta ao usuário como pedido de confirmação, com o motivo.
    Ask(RiskAssessment),
}

/// Uma chamada que roda sem perguntar.
#[derive(Debug)]
pub struct RunPlan {
    pub execution: Execution,
    /// Vem quando a chamada muda algo ou gera tráfego, para a tela dizer que
    /// rodou sozinha e por quê.
    pub risk: Option<RiskAssessment>,
}

impl RunPlan {
    /// Acesso a equipamento liberado pelo "Aceitar automaticamente" da conversa.
    #[must_use]
    pub const fn auto_accepted() -> Self {
        Self {
            execution: Execution::AutoAccepted,
            risk: None,
        }
    }
}

/// Por onde a chamada liberada executa.
#[derive(Debug)]
pub enum Execution {
    /// "Aceitar automaticamente" da conversa (ADR 012).
    AutoAccepted,
    Cleared(Clearance),
    /// Ferramenta fora do registro: o erro volta para a IA.
    Unavailable(AppError),
}

impl Execution {
    /// # Errors
    ///
    /// Ferramenta indisponível, argumento inválido ou falha da execução.
    pub async fn run(
        self,
        ctx: &AppContext,
        registry: &ToolRegistry,
        name: &str,
        args: ToolArgs,
    ) -> AppResult<ToolOutput> {
        match self {
            Self::AutoAccepted => registry.execute_auto_approved(ctx, name, args).await,
            Self::Cleared(clearance) => registry.execute_cleared(ctx, clearance, args).await,
            Self::Unavailable(error) => Err(error),
        }
    }
}

/// Decide a chamada. `arguments_json` é o texto que a IA mandou (o resumo
/// para o Decisor sai dele).
pub async fn decide(
    ctx: &AppContext,
    registry: &ToolRegistry,
    reviewer: Option<&ActionReviewer>,
    name: &str,
    args: &ToolArgs,
    arguments_json: &str,
) -> GateDecision {
    let assessment = match registry.assess(ctx, name, args).await {
        Ok(assessment) => assessment,
        Err(error) => {
            return GateDecision::Run(RunPlan {
                execution: Execution::Unavailable(error),
                risk: None,
            })
        }
    };
    let clearance = match assessment.verdict {
        Verdict::Ask => return GateDecision::Ask(assessment.risk),
        Verdict::Run(clearance) => clearance,
    };
    if matches!(assessment.kind, ToolKind::Passive | ToolKind::Interactive) {
        return GateDecision::Run(RunPlan {
            execution: Execution::Cleared(clearance),
            risk: None,
        });
    }
    if let Some(reviewer) = reviewer {
        if ActionReview::worth_reviewing(&assessment.risk)
            && decider_flags_danger(
                ctx,
                registry,
                reviewer,
                name,
                arguments_json,
                &assessment.risk,
            )
            .await
        {
            tracing::info!(tool = %name, "decisor mandou a ação do modo automático para confirmação");
            return GateDecision::Ask(RiskAssessment::escalated(ESCALATION_REASON));
        }
    }
    GateDecision::Run(RunPlan {
        execution: Execution::Cleared(clearance),
        risk: Some(assessment.risk),
    })
}

async fn decider_flags_danger(
    ctx: &AppContext,
    registry: &ToolRegistry,
    reviewer: &ActionReviewer,
    name: &str,
    arguments_json: &str,
    risk: &RiskAssessment,
) -> bool {
    let action = registry
        .preview(ctx, name, arguments_json)
        .await
        .unwrap_or_else(|_| name.to_string());
    action_review::flags_danger(
        &reviewer.runtime,
        &reviewer.settings,
        &ActionReview {
            request: &reviewer.request,
            action: &action,
            risk,
        },
    )
    .await
}
