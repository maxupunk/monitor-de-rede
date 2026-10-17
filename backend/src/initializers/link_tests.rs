//! Limpeza dos testes de enlace interrompidos — **só no processo servidor**
//! (ADR 015 §4).
//!
//! O servidor que um equipamento subiu para o teste (o `bandwidth-server`
//! sem autenticação do MikroTik, o `iperf3 -s` do OpenWrt) é desfeito pelo
//! `stop` do plugin; o teste que constava em curso vira `failed`. Roda em
//! segundo plano: o `stop` fala com o equipamento e não deve segurar o boot.
//!
//! **Ambiente de teste não dispara**: o `request_with_config` sobe o servidor
//! completo a cada teste, e a limpeza concorreria com o `truncate`. Quem a
//! exercita chama `janitor::recover` direto.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    environment::Environment,
    Result,
};

use crate::services::link_tests::{janitor, LinkTestEnv};

pub struct LinkTestsInitializer;

#[async_trait]
impl Initializer for LinkTestsInitializer {
    fn name(&self) -> String {
        "link-tests".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        if ctx.environment == Environment::Test {
            return Ok(());
        }
        let env = LinkTestEnv::from_context(ctx);
        tokio::spawn(async move {
            if let Err(error) = janitor::recover(&env).await {
                tracing::warn!(%error, "falha ao limpar testes de enlace interrompidos");
            }
        });
        Ok(())
    }
}
