//! Sobe os listeners de syslog — **só no processo servidor**.
//!
//! Um `Initializer` do Loco roda apenas no `run_app`, que é o que se quer aqui:
//! abrir porta é coisa de servidor, e um `backend-cli task …` não deve fazê-lo.
//!
//! **Só isto mora aqui.** A conexão com o banco de logs *e* a montagem do
//! pipeline (fila, escritor, barramento) vivem em `Hooks::after_context`: o
//! `run_task` não executa initializers, e com a montagem aqui um processo
//! `task scheduler_loop` ficaria sem log de aplicação nenhum. Além disso o
//! `Hooks::init_logger`, que instala a camada de `tracing`, roda **antes** dos
//! initializers — a fila precisa existir antes dele.
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
        let config = SyslogConfig::from_env();
        // O flag governa **só o listener** — quem abre porta. O banco de logs e
        // o pipeline sobem em `after_context` de qualquer forma: com
        // `SYSLOG_ENABLED=false` o que some é a escuta da rede, não o log
        // interno do servidor.
        if !config.enabled {
            tracing::info!("servidor de syslog desligado (SYSLOG_ENABLED=false)");
            return Ok(());
        }

        // **Só o socket é barrado no teste.** O `request_with_config` sobe o
        // servidor completo, initializers inclusive: sem esta trava, *todo*
        // teste de requisição tentaria abrir a 5514 e os que rodam em paralelo
        // colidiriam. A garantia é por construção — checagem de ambiente —,
        // não por lembrar de definir uma variável em cada teste. Quem exercita
        // o listener o faz direto, por `listener::spawn_udp` em porta 0.
        if ctx.environment == Environment::Test {
            return Ok(());
        }

        let Some(servico) = syslog::SyslogService::from_context(ctx) else {
            tracing::error!("pipeline de logs indisponível; listeners não abertos");
            return Ok(());
        };

        match syslog::spawn_listeners(&servico, &config).await {
            Ok((udp, tcp)) => {
                tracing::info!(
                    udp,
                    tcp,
                    "servidor de syslog no ar — publique 514:5514 no compose"
                );
            }
            Err(error) => {
                tracing::error!(%error, "não foi possível abrir as portas de syslog");
            }
        }
        Ok(())
    }
}
