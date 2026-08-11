//! Dependências de processo do domínio de monitoramento.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::{
    discovery::service::ScanSessionService, monitoring::checkers::ping::PingClient,
    network_tools::dns::registry::DnsServerRegistry,
};

/// Abre uma vez o socket ICMP compartilhado por checkers e discovery.
pub struct MonitoringInitializer;

#[async_trait]
impl Initializer for MonitoringInitializer {
    fn name(&self) -> String {
        "monitoring".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        ctx.shared_store.insert(PingClient::create()?);
        ctx.shared_store.insert(ScanSessionService::create());
        // Cadastro é uma conveniência de boot: banco indisponível não impede o
        // processo HTTP de subir, e a operação é idempotente em banco vazio.
        if let Err(error) = DnsServerRegistry::ensure_defaults(&ctx.db).await {
            tracing::warn!(%error, "não foi possível semear resolvedores DNS padrão");
        }
        Ok(())
    }
}
