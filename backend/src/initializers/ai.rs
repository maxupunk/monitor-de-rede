//! Rotinas automáticas da IA — **só no processo servidor**, e nunca em teste:
//! o `request_with_config` sobe o servidor completo, e um agendador vivo
//! disputaria o banco com cada teste.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    environment::Environment,
    Result,
};

use crate::services::ai::proactive;

pub struct AiInitializer;

#[async_trait]
impl Initializer for AiInitializer {
    fn name(&self) -> String {
        "ai".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        if ctx.environment != Environment::Test {
            proactive::spawn(ctx);
        }
        Ok(())
    }
}
