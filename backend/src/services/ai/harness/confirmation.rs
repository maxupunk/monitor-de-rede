//! Execução do que o usuário confirmou no chat.
//!
//! O chat nunca executa uma ação: devolve o pedido de confirmação e encerra a
//! vez. Confirmando, a tela chama `POST /api/ai/tools/execute`, que chega aqui
//! — com o perfil de escrita já exigido pelo guarda de rotas e o usuário
//! identificado para a auditoria.

use loco_rs::prelude::AppContext;
use serde_json::Value;

use super::tools::{ToolArgs, ToolOutput, ToolPolicy, ToolRegistry};
use crate::services::{
    ai::settings::AiSettings,
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
) -> AppResult<ToolOutput> {
    if !settings.enabled {
        return Err(AppError::validation(
            "O Assistente IA está desativado nas configurações do sistema.",
        ));
    }
    ToolRegistry::new(ToolPolicy::from_settings(settings))
        .execute_confirmed(ctx, name, ToolArgs::from_value(arguments).with_actor(actor))
        .await
}
