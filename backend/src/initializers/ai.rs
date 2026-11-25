//! Rotinas automáticas da IA — **só no processo servidor**, e nunca em teste:
//! o `request_with_config` sobe o servidor completo, e um agendador vivo
//! disputaria o banco com cada teste.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    environment::Environment,
    Result,
};

use netmonitor_assistant::proactive;
use netmonitor_core::cluster::leadership::{spawn_while_leader, LeaderRole};

pub struct AiInitializer;

#[async_trait]
impl Initializer for AiInitializer {
    fn name(&self) -> String {
        "ai".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        // Rotina do líder (ADR 024): o mesmo `alert:triggered` chega a todos
        // os processos da central, e só um pode resumi-lo.
        if ctx.environment != Environment::Test {
            spawn_while_leader(ctx, LeaderRole::Background, "ia_proativa", proactive::run);
        }
        Ok(())
    }
}
