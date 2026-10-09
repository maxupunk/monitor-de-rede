//! Como a central chega ao servidor de banco: direto ou pela ponte de um
//! agente remoto (ADR 013).
//!
//! Os drivers não sabem que a ponte existe. Com agente, o alvo devolvido
//! aponta para `127.0.0.1:<efêmera>` — a [`LocalBridge`] — e a ponte vive
//! enquanto o [`Reached`] viver. Por isso quem chama segura o `Reached` até o
//! fim do dump ou da restauração.
//!
//! O canal com o agente pode cair no meio de um dump (Wi-Fi da filial, VPN
//! que renegocia). [`Reached::retrying`] espera o agente reconectar e refaz a
//! operação — só para o que é idempotente: um dump refeito do zero sai igual;
//! uma restauração pela metade, não.

use std::{future::Future, time::Duration};

use loco_rs::app::AppContext;
use sea_orm::EntityTrait;

use super::{DatabaseError, DatabaseTarget, Progress};
use crate::{
    models::probes,
    services::{
        agents::{bridge::LocalBridge, hub::AgentHub},
        shared::errors::{AppError, AppResult},
    },
};

/// Tentativas de uma operação idempotente pela ponte, contando a primeira.
pub const ATTEMPTS: u32 = 3;

/// Quanto esperar o agente reconectar antes de desistir de uma tentativa.
const RECONNECT_WAIT: Duration = Duration::from_secs(60);

/// Um alvo pronto para conectar, com a ponte (se houver) viva junto.
pub struct Reached {
    pub target: DatabaseTarget,
    /// O agente por onde a rota passa — `None` na conexão direta.
    agent: Option<(AgentHub, i64)>,
    _bridge: Option<LocalBridge>,
}

/// Resultado de [`Reached::retrying`].
pub struct Attempted<T> {
    pub value: T,
    /// 1 quando deu certo de primeira.
    pub attempts: u32,
}

impl Reached {
    /// Executa `operation`, refazendo-a quando o canal com o agente cai.
    ///
    /// Só a rota por agente repete: na conexão direta não há canal a
    /// esperar, e a falha vai como veio. `operation` precisa ser idempotente.
    ///
    /// # Errors
    ///
    /// O erro da operação, quando não é de canal, quando as
    /// [`ATTEMPTS`] tentativas se esgotam ou quando o agente não volta.
    pub async fn retrying<T, F, Fut>(
        &self,
        progress: &dyn Progress,
        operation: F,
    ) -> Result<Attempted<T>, DatabaseError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, DatabaseError>>,
    {
        self.retrying_within(progress, RECONNECT_WAIT, operation)
            .await
    }

    async fn retrying_within<T, F, Fut>(
        &self,
        progress: &dyn Progress,
        wait: Duration,
        mut operation: F,
    ) -> Result<Attempted<T>, DatabaseError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, DatabaseError>>,
    {
        let mut attempts = 1;
        loop {
            let error = match operation().await {
                Ok(value) => return Ok(Attempted { value, attempts }),
                Err(error) => error,
            };
            let Some((hub, probe_id)) = &self.agent else {
                return Err(error);
            };
            if !error.is_transient() || attempts >= ATTEMPTS {
                return Err(error);
            }
            attempts += 1;
            tracing::warn!(%error, tentativa = attempts, "canal com o agente caiu; refazendo");
            progress.stage(&format!(
                "O canal com o agente caiu — tentando de novo ({attempts}/{ATTEMPTS})"
            ));
            if hub.wait_for(*probe_id, wait).await.is_none() {
                return Err(error);
            }
        }
    }
}

/// Resolve a rota de `target`. Sem `via_probe_id`, devolve o próprio alvo.
///
/// # Errors
///
/// Agente desconectado, sem a permissão `database`, sem o destino na lista
/// local, ou que não alcançou o banco — com a mensagem do agente.
pub async fn reach(
    ctx: &AppContext,
    mut target: DatabaseTarget,
    via_probe_id: Option<i64>,
) -> AppResult<Reached> {
    let Some(probe_id) = via_probe_id else {
        return Ok(Reached {
            target,
            agent: None,
            _bridge: None,
        });
    };
    let name = probes::Entity::find_by_id(probe_id)
        .one(&ctx.db)
        .await?
        .map_or_else(|| format!("#{probe_id}"), |probe| probe.name);
    let hub = AgentHub::from_context(ctx)
        .ok()
        .filter(|hub| hub.get(probe_id).is_some())
        .ok_or_else(|| {
            AppError::business_rule(format!(
                "O agente {name} não está conectado — esta conexão de banco passa por ele"
            ))
        })?;
    let bridge = LocalBridge::start(hub.clone(), probe_id, target.host.clone(), target.port)
        .await
        .map_err(|error| {
            AppError::business_rule(format!("Ponte pelo agente {name}: {}", error.message))
        })?;
    tracing::info!(
        agente = %name,
        destino = %format!("{}:{}", target.host, target.port),
        "ponte de banco aberta pelo agente"
    );
    target.host = bridge.addr().ip().to_string();
    target.port = bridge.addr().port();
    target.via = Some(name);
    Ok(Reached {
        target,
        agent: Some((hub, probe_id)),
        _bridge: Some(bridge),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    use tokio::sync::mpsc;

    use super::*;
    use crate::services::{
        agents::{policy::Permission, session::tests::hello, session::AgentSession},
        databases::NoProgress,
    };

    fn target() -> DatabaseTarget {
        DatabaseTarget {
            engine: super::super::DatabaseEngine::Postgres,
            host: "127.0.0.1".into(),
            port: 5432,
            username: "u".into(),
            password: String::new(),
            ssl_mode: super::super::SslMode::Disable,
            database: None,
            via: None,
        }
    }

    fn session(hub: &AgentHub) -> Arc<AgentSession> {
        let (tx, _rx) = mpsc::channel(1);
        let session = Arc::new(AgentSession::new(
            7,
            "filial",
            hello(vec![Permission::Database]),
            tx,
        ));
        hub.register(session.clone());
        session
    }

    fn via_agent(hub: &AgentHub) -> Reached {
        Reached {
            target: target(),
            agent: Some((hub.clone(), 7)),
            _bridge: None,
        }
    }

    fn dropped() -> DatabaseError {
        DatabaseError::Dropped("connection reset".into())
    }

    #[tokio::test]
    async fn pela_ponte_refaz_depois_que_o_agente_volta() {
        let hub = AgentHub::default();
        let first = session(&hub);
        let reached = via_agent(&hub);
        let calls = AtomicU32::new(0);
        let result = reached
            .retrying_within(&NoProgress, Duration::from_secs(5), || {
                let call = calls.fetch_add(1, Ordering::SeqCst);
                let (hub, first) = (hub.clone(), first.clone());
                async move {
                    if call == 0 {
                        // O canal cai e o agente reconecta logo depois.
                        first.close();
                        tokio::spawn(async move { session(&hub) });
                        return Err(dropped());
                    }
                    Ok("dump")
                }
            })
            .await
            .expect("segunda tentativa");
        assert_eq!((result.value, result.attempts), ("dump", 2));
    }

    #[tokio::test]
    async fn desiste_depois_das_tentativas() {
        let hub = AgentHub::default();
        session(&hub);
        let calls = AtomicU32::new(0);
        let result = via_agent(&hub)
            .retrying_within(&NoProgress, Duration::from_secs(5), || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(dropped()) }
            })
            .await;
        assert!(matches!(result, Err(DatabaseError::Dropped(_))));
        assert_eq!(calls.load(Ordering::SeqCst), ATTEMPTS);
    }

    #[tokio::test]
    async fn agente_que_nao_volta_encerra_na_hora_da_espera() {
        let hub = AgentHub::default();
        session(&hub).close();
        let calls = AtomicU32::new(0);
        let result = via_agent(&hub)
            .retrying_within(&NoProgress, Duration::from_millis(50), || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(dropped()) }
            })
            .await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn erro_de_conteudo_e_conexao_direta_nao_repetem() {
        let hub = AgentHub::default();
        session(&hub);
        for (reached, error) in [
            (via_agent(&hub), DatabaseError::Connection("senha".into())),
            (via_agent(&hub), DatabaseError::Query("sintaxe".into())),
            (
                Reached {
                    target: target(),
                    agent: None,
                    _bridge: None,
                },
                dropped(),
            ),
        ] {
            let calls = AtomicU32::new(0);
            let message = error.to_string();
            let mut error = Some(error);
            let result = reached
                .retrying_within(&NoProgress, Duration::from_secs(5), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let error = error.take().unwrap_or_else(dropped);
                    async move { Err::<(), _>(error) }
                })
                .await;
            assert_eq!(result.err().map(|e| e.to_string()), Some(message));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }
}
