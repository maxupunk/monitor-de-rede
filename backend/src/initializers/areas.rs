//! Publica o estado das áreas do produto no boot do servidor (ADR 017).
//!
//! O snapshot fica guardado no barramento: toda aba que conectar ao SSE recebe
//! `areas:updated` logo depois de `stream:connected` e monta o menu sem
//! nenhuma chamada HTTP (AGENTS §9). Mudanças posteriores são publicadas pelo
//! `PUT /api/areas/{area}`.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};
use netmonitor_core::areas;

use crate::services::events::EventBus;

pub struct AreasInitializer;

#[async_trait]
impl Initializer for AreasInitializer {
    fn name(&self) -> String {
        "areas".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        let policies = crate::registries::area_policies();
        match (
            areas::snapshot(&ctx.db, &policies).await,
            EventBus::from_context(ctx),
        ) {
            (Ok(snapshot), Ok(bus)) => areas::publish(&bus, &snapshot),
            (Err(error), _) | (_, Err(error)) => {
                tracing::warn!(%error, "não foi possível publicar o estado das áreas no boot");
            }
        }
        Ok(())
    }
}
