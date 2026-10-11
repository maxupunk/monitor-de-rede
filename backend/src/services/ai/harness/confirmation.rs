//! Execução do que o usuário confirmou no chat.
//!
//! O chat nunca executa uma ação: devolve o pedido de confirmação e encerra a
//! vez. Confirmando, a tela chama `POST /api/ai/tools/execute`, que chega aqui
//! — com o perfil de escrita já exigido pelo guarda de rotas e o usuário
//! identificado para a auditoria.
//!
//! O pedido também fica no balcão de pendências (`ai::approvals`), para
//! aparecer em qualquer tela e avisar quem não está olhando. Decidir aqui o
//! tira de lá — e a mesma proposta, decidida numa aba, não roda na outra.

use loco_rs::prelude::AppContext;
use serde_json::Value;

use super::tools::{ToolArgs, ToolOutput, ToolPolicy, ToolRegistry};
use crate::services::{
    ai::{
        approvals::{self, Decision, Proposal, Settled},
        settings::AiSettings,
    },
    audit::AuditActor,
    shared::errors::{AppError, AppResult},
};

/// Executa a ferramenta confirmada, respeitando as configurações atuais: se
/// o administrador desligou as ações entre a proposta e o clique, ela não roda.
///
/// # Errors
///
/// Assistente desativado, ferramenta indisponível ou passiva, argumento
/// inválido ou falha da execução.
pub async fn execute_confirmed(
    ctx: &AppContext,
    settings: &AiSettings,
    name: &str,
    arguments: Value,
    actor: AuditActor,
    conversation: Option<String>,
) -> AppResult<ToolOutput> {
    if !settings.enabled {
        return Err(AppError::validation(
            "O Assistente IA está desativado nas configurações do sistema.",
        ));
    }
    ToolRegistry::new(ToolPolicy::from_settings(settings))
        .execute_confirmed(
            ctx,
            name,
            ToolArgs::from_value(arguments)
                .with_actor(actor)
                .with_conversation(conversation),
        )
        .await
}

/// Confirmação pelo card do chat: tira a pendência do balcão (recusando a já
/// decidida em outra tela) e executa a chamada do card.
///
/// # Errors
///
/// Proposta já decidida, ou os mesmos de [`execute_confirmed`].
pub async fn confirm_in_chat(
    ctx: &AppContext,
    settings: &AiSettings,
    call_id: Option<&str>,
    name: &str,
    arguments: Value,
    actor: AuditActor,
    conversation: Option<String>,
) -> AppResult<ToolOutput> {
    if let (Some(call_id), Some(user_id)) = (call_id, actor.user_id) {
        approvals::ensure_undecided(&approvals::settle(
            ctx,
            user_id,
            call_id,
            Decision::Confirmed,
        ))?;
    }
    execute_confirmed(ctx, settings, name, arguments, actor, conversation).await
}

/// Confirmação fora do chat (o aviso em outra tela): executa a proposta
/// guardada no balcão, não uma cópia vinda da tela.
///
/// # Errors
///
/// Sem usuário, pedido inexistente, expirado ou já decidido, ou os mesmos de
/// [`execute_confirmed`].
pub async fn confirm_pending(
    ctx: &AppContext,
    settings: &AiSettings,
    call_id: &str,
    actor: AuditActor,
) -> AppResult<ToolOutput> {
    let user_id = actor
        .user_id
        .ok_or_else(|| AppError::unauthorized("Não autenticado"))?;
    match approvals::settle(ctx, user_id, call_id, Decision::Confirmed) {
        Settled::Pending(proposal) => {
            let Proposal {
                approval,
                arguments,
                conversation_key,
            } = *proposal;
            execute_confirmed(
                ctx,
                settings,
                &approval.tool,
                arguments,
                actor,
                conversation_key,
            )
            .await
        }
        Settled::AlreadyDecided => Err(approvals::already_decided()),
        Settled::Unknown => Err(AppError::not_found(
            "Pedido de aprovação não encontrado ou expirado; confirme pelo card da conversa.",
        )),
    }
}

/// O usuário cancelou: a pendência sai de todas as telas. Sem pendência
/// (reinício, outro usuário), nada a fazer.
pub fn dismiss(ctx: &AppContext, actor: &AuditActor, call_id: &str) {
    if let Some(user_id) = actor.user_id {
        let _ = approvals::settle(ctx, user_id, call_id, Decision::Dismissed);
    }
}
