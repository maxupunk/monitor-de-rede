//! Quem **dirige** um teste nativo (o lado cliente) — a central ou um agente.
//!
//! O orquestrador do teste de enlace (ADR 015 §5) não sabe de onde o teste
//! parte: pede a um [`ThroughputDriver`] e recebe as amostras e o relatório.
//! [`LocalDriver`] dirige deste processo; o agente remoto implementa o mesmo
//! contrato pelo `Command::Throughput` (ADR 015 §6).

use std::net::SocketAddr;

use agent_protocol::{Command, ThroughputAuth};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{btest, iperf3, SampleSink, ThroughputEngine, ThroughputReport, ThroughputSpec};
use crate::services::shared::errors::AppResult;

/// Um teste a dirigir: alvo, especificação e, para o btest, a credencial do
/// teste (gerada pelo orquestrador; `None` = servidor sem autenticação).
#[derive(Debug, Clone)]
pub struct DriveRequest {
    pub target: SocketAddr,
    pub spec: ThroughputSpec,
    pub auth: Option<btest::BtestAuth>,
}

/// O contrato que o orquestrador conhece.
#[async_trait]
pub trait ThroughputDriver: Send + Sync {
    /// Dirige o teste até o fim, mandando uma amostra por segundo ao `sink`.
    ///
    /// # Errors
    ///
    /// Os mesmos dos motores: alvo inalcançável, protocolo violado, servidor
    /// ocupado, cancelamento.
    async fn drive(
        &self,
        request: &DriveRequest,
        sink: &SampleSink,
        cancel: &CancellationToken,
    ) -> AppResult<ThroughputReport>;

    /// De onde o teste parte, para a tela ("central", "agente Filial").
    fn origin(&self) -> String;
}

/// Dirige a partir deste processo.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalDriver;

#[async_trait]
impl ThroughputDriver for LocalDriver {
    async fn drive(
        &self,
        request: &DriveRequest,
        sink: &SampleSink,
        cancel: &CancellationToken,
    ) -> AppResult<ThroughputReport> {
        match request.spec.engine {
            ThroughputEngine::Iperf3 => {
                iperf3::run_client(request.target, &request.spec, sink, cancel).await
            }
            ThroughputEngine::Btest => {
                btest::run_client(
                    request.target,
                    &request.spec,
                    request.auth.as_ref(),
                    sink,
                    cancel,
                )
                .await
            }
        }
    }

    fn origin(&self) -> String {
        "central".to_owned()
    }
}

/// A credencial do teste no fio (`Command::Throughput`) e de volta. Moram do
/// lado do motor: o contrato de fio não conhece o btest.
impl From<&btest::BtestAuth> for ThroughputAuth {
    fn from(auth: &btest::BtestAuth) -> Self {
        Self {
            user: auth.user.clone(),
            password: auth.password.clone(),
        }
    }
}

impl From<ThroughputAuth> for btest::BtestAuth {
    fn from(auth: ThroughputAuth) -> Self {
        Self {
            user: auth.user,
            password: auth.password,
        }
    }
}

/// O pedido do orquestrador vira o comando ao agente que vai dirigi-lo.
impl From<&DriveRequest> for Command {
    fn from(request: &DriveRequest) -> Self {
        Self::Throughput {
            target: request.target.ip(),
            port: request.target.port(),
            spec: request.spec.clone(),
            auth: request.auth.as_ref().map(ThroughputAuth::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_protocol::{Body, Envelope, Outcome};
    use uuid::Uuid;

    use super::*;
    use crate::services::network_tools::throughput::{TestDirection, TestProtocol};

    #[test]
    fn pedido_do_motor_vira_comando_de_vazao() {
        let request = DriveRequest {
            target: "192.168.88.1:2000".parse().unwrap(),
            spec: ThroughputSpec {
                engine: ThroughputEngine::Btest,
                direction: TestDirection::Both,
                protocol: TestProtocol::Tcp,
                duration_s: 5,
                streams: 1,
                limit_mbps: Some(100),
            },
            auth: None,
        };
        let Command::Throughput {
            target, port, auth, ..
        } = Command::from(&request)
        else {
            panic!("vazão");
        };
        assert_eq!(target.to_string(), "192.168.88.1");
        assert_eq!(port, 2000);
        assert!(auth.is_none());
    }

    #[test]
    fn credencial_do_btest_faz_ida_e_volta_pelo_fio() {
        let auth = btest::BtestAuth {
            user: "nm-teste".into(),
            password: "s3nh4-do-teste".into(),
        };
        let wire = ThroughputAuth::from(&auth);
        assert_eq!(wire.user, "nm-teste");
        let back: btest::BtestAuth = wire.into();
        assert_eq!(back.password, "s3nh4-do-teste");
    }

    #[test]
    fn amostra_e_relatorio_viajam_em_chunk_e_resposta() {
        use super::super::{Flow, ThroughputSample};
        let sample = ThroughputSample {
            t_ms: 1_000,
            flow: Flow::Rx,
            bps: 94_500_000.0,
        };
        let chunk = Envelope::reply(
            Uuid::nil(),
            Body::Chunk {
                data: serde_json::to_value(sample).unwrap(),
            },
        );
        let Body::Chunk { data } = Envelope::decode(&chunk.encode().unwrap()).unwrap().body else {
            panic!("chunk");
        };
        assert_eq!(
            serde_json::from_value::<ThroughputSample>(data).unwrap(),
            sample
        );

        let mut report = ThroughputReport::empty(ThroughputEngine::Iperf3);
        report.samples.push(sample);
        let reply = Envelope::reply(
            Uuid::nil(),
            Body::Response {
                outcome: Outcome::Ok {
                    value: serde_json::to_value(&report).unwrap(),
                },
            },
        );
        let Body::Response {
            outcome: Outcome::Ok { value },
        } = Envelope::decode(&reply.encode().unwrap()).unwrap().body
        else {
            panic!("resposta");
        };
        assert_eq!(
            serde_json::from_value::<ThroughputReport>(value).unwrap(),
            report
        );
    }
}
