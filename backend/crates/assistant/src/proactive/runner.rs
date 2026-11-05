//! Pergunta ao agente sem tela: mesmo loop do chat, só com ferramentas de
//! leitura e resposta no modo direto.

use async_trait::async_trait;
use loco_rs::prelude::AppContext;

use crate::{
    drivers::{self, traits::AiDriver},
    dtos::ChatMessageInput,
    harness::{
        agent::{
            collect_answer, run_agent_loop, AgentAnswer, AgentRequest, ConversationMemory,
            ToolLoading,
        },
        prompt::ChatContext,
        tools::ToolPolicy,
    },
    response_style::AiResponseStyle,
    settings::AiSettings,
};
use netmonitor_core::shared::errors::AppResult;

/// De onde vem o driver. Em produção, do provedor configurado; nos testes,
/// de um driver que responde sem rede.
#[async_trait]
pub trait DriverFactory: Send + Sync {
    /// # Errors
    ///
    /// Provedor não configurado.
    async fn create(&self, settings: &AiSettings) -> AppResult<Box<dyn AiDriver>>;
}

/// O provedor escolhido nas configurações.
pub struct ProviderDrivers;

#[async_trait]
impl DriverFactory for ProviderDrivers {
    async fn create(&self, settings: &AiSettings) -> AppResult<Box<dyn AiDriver>> {
        drivers::connect(settings).await
    }
}

/// Faz uma pergunta ao agente e devolve a resposta inteira.
///
/// Rotinas automáticas nunca executam teste ativo nem ação: ninguém está
/// olhando para confirmar.
///
/// # Errors
///
/// Provedor indisponível, erro no stream ou resposta vazia.
pub async fn ask(
    ctx: &AppContext,
    settings: &AiSettings,
    drivers: &dyn DriverFactory,
    prompt: String,
    context: ChatContext,
) -> AppResult<AgentAnswer> {
    let mut settings = settings.clone();
    settings.response_style = AiResponseStyle::Concise;
    let driver = drivers.create(&settings).await?;
    let stream = run_agent_loop(
        ctx.clone(),
        settings,
        driver,
        AgentRequest {
            messages: vec![ChatMessageInput {
                role: "user".into(),
                content: prompt,
            }],
            context,
            policy: ToolPolicy::passive(),
            tool_loading: ToolLoading::Everything,
            memory: ConversationMemory::default(),
            actor: netmonitor_core::audit::AuditActor::default(),
        },
    )
    .await;
    collect_answer(stream).await
}
