//! Sobe os listeners de syslog — **só no processo servidor**.
//!
//! Um `Initializer` do Loco roda apenas no `run_app`, que é o que se quer aqui:
//! abrir porta é coisa de servidor, e um `backend-cli task …` não deve fazê-lo.
//! A conexão com o banco de logs, essa sim, vive em `Hooks::after_context`,
//! porque a purga de retenção roda no ciclo do `scheduler` (ADR 007).
//!
//! **Ambiente de teste nunca escuta.** O `request_with_config` sobe o servidor
//! completo, initializers inclusive: sem esta trava, *todo* teste de requisição
//! tentaria abrir a 5514 e os que rodam em paralelo colidiriam entre si. A
//! garantia é por construção — checagem de ambiente —, não por lembrar de
//! definir uma variável em cada teste. Quem exercita o listener o faz direto,
//! por `services::syslog::listener::spawn_udp` em porta 0.

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    environment::Environment,
    Result,
};

use crate::services::syslog::{self, SyslogConfig};

pub struct SyslogInitializer;

#[async_trait]
impl Initializer for SyslogInitializer {
    fn name(&self) -> String {
        "syslog".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        if ctx.environment == Environment::Test {
            return Ok(());
        }
        let config = SyslogConfig::from_env();
        if !config.enabled {
            tracing::info!("servidor de syslog desligado (SYSLOG_ENABLED=false)");
            return Ok(());
        }

        // Falha aqui **não** derruba o boot: porta ocupada ou banco de logs
        // indisponível não podem impedir o monitoramento e a interface web de
        // subirem. O erro fica no log e a tela de logs mostra o serviço parado.
        match syslog::start(ctx, config).await {
            Ok((udp, tcp)) => {
                tracing::info!(
                    udp,
                    tcp,
                    "servidor de syslog no ar — publique 514:5514 no compose"
                );
            }
            Err(error) => {
                tracing::error!(%error, "não foi possível subir o servidor de syslog");
            }
        }
        Ok(())
    }
}
