//! Encaminha a E/S do plugin a um agente remoto (ADR 011).
//!
//! O agente só disca para fora: a central não abre conexão com ele, usa o
//! canal que ele mantém. E o agente decide — `Permission::DeviceIo` precisa
//! estar no `AGENT_ALLOW` do host, e o alvo precisa ser da rede privada. A
//! checagem de permissão aqui só evita mandar um pedido que seria recusado.
//!
//! Desde o protocolo 4 (ADR 015, §6) o agente manda cada linha da saída num
//! `Chunk` enquanto o comando roda: `execute_streaming` as repassa a `on_line`
//! na hora, e a resposta final continua a completa.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{DeviceIoCall, DeviceIoReply, DeviceTransport, LineSink};
use crate::services::{
    agents::{policy::Permission, protocol::Command, session::AgentSession},
    shared::errors::{AppError, AppResult},
};

/// Folga sobre o timeout da própria chamada para a volta pelo túnel.
const GRACE: Duration = Duration::from_secs(10);

pub struct AgentTransport {
    session: Arc<AgentSession>,
}

impl AgentTransport {
    /// # Errors
    ///
    /// O agente não anunciou `device_io` na política.
    pub fn new(session: Arc<AgentSession>) -> AppResult<Self> {
        if !session.allows(Permission::DeviceIo) {
            return Err(AppError::business_rule(format!(
                "O agente {} não libera acesso a equipamentos. Inclua `device_io` no AGENT_ALLOW \
                 daquele host para usar plugins pela rede dele.",
                session.name
            )));
        }
        Ok(Self { session })
    }

    /// Executa pelo agente, entregando a `on_line` (quando houver) cada linha
    /// que chegar em `Chunk`.
    async fn run(
        &self,
        call: &DeviceIoCall,
        on_line: Option<LineSink<'_>>,
    ) -> AppResult<DeviceIoReply> {
        let timeout = Duration::from_millis(call.timeout_ms()) + GRACE;
        let value = self
            .session
            .stream(
                Command::DeviceIo { call: call.clone() },
                timeout,
                &CancellationToken::new(),
                |chunk| {
                    if let (Some(on_line), Some(line)) = (on_line, chunk.as_str()) {
                        on_line(line);
                    }
                },
            )
            .await
            .map_err(|error| {
                AppError::business_rule(format!("Agente {}: {error}", self.session.name))
            })?;
        serde_json::from_value(value).map_err(|error| {
            AppError::Internal(anyhow::anyhow!("resposta inválida do agente: {error}"))
        })
    }
}

#[async_trait]
impl DeviceTransport for AgentTransport {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        self.run(call, None).await
    }

    async fn execute_streaming(
        &self,
        call: &DeviceIoCall,
        on_line: LineSink<'_>,
    ) -> AppResult<DeviceIoReply> {
        self.run(call, Some(on_line)).await
    }

    fn origin(&self) -> String {
        format!("agente {}", self.session.name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;
    use tokio::sync::mpsc;

    use super::*;
    use crate::services::{
        agents::{
            protocol::{Body, Envelope, Outcome},
            session::tests::hello,
        },
        plugins::transport::{Endpoint, Login},
    };

    fn ssh(command: &str) -> DeviceIoCall {
        DeviceIoCall::SshExec {
            endpoint: Endpoint {
                host: "192.168.88.1".into(),
                port: 22,
            },
            login: Login {
                username: "admin".into(),
                password: Some("s3nh4".into()),
            },
            command: command.into(),
            timeout_ms: 5_000,
        }
    }

    /// Agente falso: duas linhas em `Chunk` e a resposta completa.
    fn agent() -> (AgentTransport, tokio::task::JoinHandle<()>) {
        let (tx, mut outbound) = mpsc::channel(8);
        let session = Arc::new(AgentSession::new(
            7,
            "filial",
            hello(vec![Permission::DeviceIo]),
            tx,
        ));
        let replier = {
            let session = session.clone();
            tokio::spawn(async move {
                while let Some(request) = outbound.recv().await {
                    if !matches!(request.body, Body::Request { .. }) {
                        continue;
                    }
                    for line in ["tx-current: 94.5Mbps", "rx-current: 10Mbps"] {
                        session.handle(Envelope::reply(
                            request.id,
                            Body::Chunk { data: json!(line) },
                        ));
                    }
                    let reply = DeviceIoReply::Exec {
                        stdout: "tx-current: 94.5Mbps\nrx-current: 10Mbps\n".into(),
                        stderr: String::new(),
                        exit_code: Some(0),
                    };
                    session.handle(Envelope::reply(
                        request.id,
                        Body::Response {
                            outcome: Outcome::Ok {
                                value: serde_json::to_value(reply).unwrap(),
                            },
                        },
                    ));
                }
            })
        };
        (AgentTransport::new(session).expect("device_io"), replier)
    }

    #[tokio::test]
    async fn linhas_do_agente_chegam_enquanto_o_comando_roda() {
        let (transport, replier) = agent();
        let lines = Mutex::new(Vec::new());
        let reply = transport
            .execute_streaming(&ssh("/tool bandwidth-test"), &|line: &str| {
                lines.lock().unwrap().push(line.to_owned());
            })
            .await
            .expect("resposta");
        assert_eq!(
            lines.into_inner().unwrap(),
            ["tx-current: 94.5Mbps", "rx-current: 10Mbps"]
        );
        let DeviceIoReply::Exec { stdout, .. } = &reply else {
            panic!("exec");
        };
        assert!(stdout.starts_with("tx-current"));

        // Quem não quer as linhas recebe a mesma resposta completa.
        assert_eq!(transport.execute(&ssh("uptime")).await.expect("ok"), reply);
        assert_eq!(transport.origin(), "agente filial");
        replier.abort();
    }

    #[test]
    fn sem_device_io_nao_cria_o_transporte() {
        let (tx, _rx) = mpsc::channel(1);
        let session = Arc::new(AgentSession::new(
            7,
            "filial",
            hello(vec![Permission::Read]),
            tx,
        ));
        assert!(AgentTransport::new(session).is_err());
    }
}
