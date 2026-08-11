//! Dependências de processo do domínio de monitoramento.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::{
    alerts::catalog::service as alert_catalog, discovery::service::ScanSessionService,
    events::EventBus, monitoring::checkers::ping::PingClient,
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
        ctx.shared_store.insert(EventBus::create());
        // Cadastro é uma conveniência de boot: banco indisponível não impede o
        // processo HTTP de subir, e a operação é idempotente em banco vazio.
        if let Err(error) = DnsServerRegistry::ensure_defaults(&ctx.db).await {
            tracing::warn!(%error, "não foi possível semear resolvedores DNS padrão");
        }
        // Provisiona o conjunto básico de regras em instalação nova (§9.5).
        // Falha aqui **não** impede o boot: o banco pode estar migrando, e a
        // API precisa subir de qualquer forma. A operação é idempotente e só
        // age quando não existe regra alguma.
        match alert_catalog::ensure_defaults(&ctx.db).await {
            Ok(result) if !result.created.is_empty() => {
                tracing::info!(
                    created = result.created.len(),
                    "regras básicas aplicadas a partir do catálogo"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "não foi possível provisionar as regras básicas de alerta");
            }
        }
        Ok(())
    }
}
