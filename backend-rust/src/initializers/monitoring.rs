//! Dependências de processo do domínio de monitoramento.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::monitoring::checkers::ping::PingClient;

/// Abre uma vez o socket ICMP compartilhado por checkers e discovery.
pub struct MonitoringInitializer;

#[async_trait]
impl Initializer for MonitoringInitializer {
    fn name(&self) -> String {
        "monitoring".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        ctx.shared_store.insert(PingClient::create()?);
        Ok(())
    }
}
