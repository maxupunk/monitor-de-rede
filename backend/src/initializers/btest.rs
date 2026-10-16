//! Abre o servidor btest da central — **só no processo servidor** (ADR 015).
//!
//! Abrir porta é coisa de servidor, como o syslog: um `backend-cli task …`
//! não deve fazê-lo, e o `run_task` não executa initializers. Quem precisa do
//! servidor o pega com `BtestServer::from_context`, que devolve o erro
//! nomeado "servidor btest indisponível" quando ele não existe neste processo.
//!
//! **Nunca derruba o boot.** Porta ocupada, sem permissão ou `BTEST_LISTEN`
//! inválido viram aviso no log; o resto do NetMonitor sobe normalmente.
//!
//! **Ambiente de teste nunca escuta**, pelo mesmo motivo do syslog: o
//! `request_with_config` sobe o servidor completo, initializers inclusive, e
//! os testes em paralelo colidiriam na 2000. Quem exercita o servidor o faz
//! direto, por `BtestServer::bind` em `127.0.0.1:0`.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    environment::Environment,
    Result,
};

use crate::services::network_tools::throughput::btest::{self, BtestServer};

pub struct BtestInitializer;

#[async_trait]
impl Initializer for BtestInitializer {
    fn name(&self) -> String {
        "btest".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        if ctx.environment == Environment::Test {
            return Ok(());
        }
        let addr = match btest::listen_addr_from_env() {
            Ok(Some(addr)) => addr,
            Ok(None) => {
                tracing::info!("servidor btest desligado ({}= vazio)", btest::LISTEN_ENV);
                return Ok(());
            }
            Err(problem) => {
                tracing::warn!(%problem, "servidor btest não aberto");
                return Ok(());
            }
        };
        match BtestServer::bind(addr).await {
            Ok(server) => {
                server.install(ctx);
                tracing::info!(
                    addr = %server.local_addr(),
                    "servidor btest no ar — só atende sessões armadas"
                );
            }
            Err(error) => {
                tracing::warn!(%error, "servidor btest indisponível; testes de enlace com MikroTik → central não vão rodar");
            }
        }
        Ok(())
    }
}
