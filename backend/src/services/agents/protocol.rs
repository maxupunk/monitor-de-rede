//! Protocolo do canal central ↔ agente (ADR 011).
//!
//! Tudo o que atravessa o WebSocket é um [`Envelope`] em JSON. Um pedido da
//! central (`Request`) recebe zero ou mais `Chunk` com o mesmo `id` (progresso,
//! linhas de log em follow) e termina em exatamente um `Response`. `Cancel`
//! interrompe um pedido em andamento. Eventos (`Event`) são espontâneos do
//! agente: snapshots ao vivo, rollups de métricas, resultados que ficaram
//! presos no buffer offline.
//!
//! Os dados de uma ponte de banco (`Command::DatabaseTunnel`, ADR 013) não
//! viajam em JSON: vão em quadros **binários** ([`TunnelFrame`]) com o `id` do
//! pedido, nos dois sentidos, sob uma janela de crédito. O texto continua
//! sendo o canal de controle.
//!
//! Os comandos Docker formam um **enum fechado**. Não existe "encaminhe este
//! caminho para a Engine": cada operação é declarada, tem permissão própria
//! ([`Command::permission`]) e é validada dos dois lados.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    services::{
        docker::{
            engine::{ContainerAction, LogFilters},
            maintenance::ComposeRequest,
            DockerError,
        },
        monitoring::contracts::CheckResult,
        plugins::transport::DeviceIoCall,
        probes::{
            dispatcher::{ProbeDiscoveryTask, ProbeTask},
            receiver::ProbeDiscoveryResultPayload,
        },
        shared::errors::AppError,
        telemetry::rollup::MetricsRollup,
    },
    views::docker::{DockerInventorySnapshot, DockerLiveSnapshot},
};

use super::policy::Permission;

/// Versão do protocolo. Muda quando um lado deixaria de entender o outro, e o
/// handshake recusa versão diferente: central e agente sobem juntos.
///
/// * **1** — texto JSON só;
/// * **2** — quadros binários de ponte ([`TunnelFrame`]) e
///   `Command::DatabaseTunnel` (ADR 013).
pub const PROTOCOL_VERSION: u16 = 2;

/// Teto de um quadro. Logs e inventários grandes são quebrados em chunks.
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// Um quadro do WebSocket, como os dois lados o trocam com o socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireFrame {
    /// Um [`Envelope`] em JSON.
    Text(String),
    /// Um [`TunnelFrame`] codificado.
    Binary(Vec<u8>),
}

/// Teto dos dados de um quadro de ponte.
pub const TUNNEL_DATA_MAX: usize = 64 * 1024;

/// O que um quadro de ponte carrega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelPayload {
    /// Bytes da conexão.
    Data(Vec<u8>),
    /// Crédito devolvido: quem recebe pode mandar mais estes bytes.
    Ack(u32),
    /// Quem mandou não vai escrever mais (meio-fechamento).
    Eof,
}

/// Quadro binário de uma ponte: `[id 16 bytes][tipo 1 byte][conteúdo]`.
///
/// Binário e não JSON: o dado de um banco atravessaria em base64, um terço
/// maior, e passaria duas vezes pelo serializador.
///
/// Dados podem ir crus ou comprimidos com zstd ([`TunnelFrame::encode_data`]);
/// quem recebe não precisa saber qual — [`TunnelFrame::decode`] devolve sempre
/// os bytes originais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelFrame {
    pub id: Uuid,
    pub payload: TunnelPayload,
}

const KIND_DATA: u8 = 0;
const KIND_ACK: u8 = 1;
const KIND_EOF: u8 = 2;
const KIND_DATA_ZSTD: u8 = 3;

/// Nível do zstd: o mais rápido. A ponte comprime em tempo real, no caminho de
/// um dump; ganhar uns pontos de taxa não vale segurar o banco.
const ZSTD_LEVEL: i32 = 1;

impl TunnelFrame {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(17 + TUNNEL_DATA_MAX);
        out.extend_from_slice(self.id.as_bytes());
        match &self.payload {
            TunnelPayload::Data(bytes) => {
                out.push(KIND_DATA);
                out.extend_from_slice(bytes);
            }
            TunnelPayload::Ack(credit) => {
                out.push(KIND_ACK);
                out.extend_from_slice(&credit.to_be_bytes());
            }
            TunnelPayload::Eof => out.push(KIND_EOF),
        }
        out
    }

    /// Quadro de dados comprimido, se a compressão compensar.
    ///
    /// Devolve `None` quando o resultado não fica ao menos 10% menor — dado já
    /// cifrado (banco com TLS) ou já compactado não comprime, e mandar cru
    /// poupa o outro lado de descomprimir à toa.
    #[must_use]
    pub fn encode_compressed(id: Uuid, data: &[u8]) -> Option<Vec<u8>> {
        let packed = zstd::bulk::compress(data, ZSTD_LEVEL).ok()?;
        if packed.len() * 10 > data.len() * 9 {
            return None;
        }
        let mut out = Vec::with_capacity(17 + packed.len());
        out.extend_from_slice(id.as_bytes());
        out.push(KIND_DATA_ZSTD);
        out.extend_from_slice(&packed);
        Some(out)
    }

    /// # Errors
    ///
    /// Quadro curto, tipo desconhecido, dados acima de [`TUNNEL_DATA_MAX`] —
    /// também depois de descomprimidos — ou compressão inválida.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let malformed = |why: &str| ProtocolError::Malformed(format!("quadro de ponte {why}"));
        if bytes.len() < 17 {
            return Err(malformed("curto demais"));
        }
        let id = Uuid::from_slice(&bytes[..16]).map_err(|_| malformed("sem id"))?;
        let rest = &bytes[17..];
        let payload = match bytes[16] {
            KIND_DATA if rest.len() <= TUNNEL_DATA_MAX => TunnelPayload::Data(rest.to_vec()),
            KIND_DATA => return Err(ProtocolError::FrameTooLarge),
            KIND_ACK => TunnelPayload::Ack(u32::from_be_bytes(
                rest.try_into()
                    .map_err(|_| malformed("com crédito inválido"))?,
            )),
            KIND_EOF if rest.is_empty() => TunnelPayload::Eof,
            // O teto vale para o descomprimido: um quadro pequeno não pode
            // virar gigabytes do outro lado.
            KIND_DATA_ZSTD => TunnelPayload::Data(
                zstd::bulk::decompress(rest, TUNNEL_DATA_MAX)
                    .map_err(|_| malformed("com compressão inválida ou grande demais"))?,
            ),
            _ => return Err(malformed("de tipo desconhecido")),
        };
        Ok(Self { id, payload })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub v: u16,
    pub id: Uuid,
    pub body: Body,
}

impl Envelope {
    #[must_use]
    pub fn new(body: Body) -> Self {
        Self::reply(Uuid::new_v4(), body)
    }

    /// Mesma correlação de um pedido anterior.
    #[must_use]
    pub const fn reply(id: Uuid, body: Body) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id,
            body,
        }
    }

    /// # Errors
    ///
    /// Quadro acima de [`MAX_FRAME_BYTES`] ou JSON inválido.
    pub fn decode(text: &str) -> Result<Self, ProtocolError> {
        if text.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        serde_json::from_str(text).map_err(|error| ProtocolError::Malformed(error.to_string()))
    }

    /// # Errors
    ///
    /// Quadro que passaria de [`MAX_FRAME_BYTES`].
    pub fn encode(&self) -> Result<String, ProtocolError> {
        let text = serde_json::to_string(self)
            .map_err(|error| ProtocolError::Malformed(error.to_string()))?;
        if text.len() > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("quadro acima do limite do protocolo")]
    FrameTooLarge,
    #[error("quadro inválido: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Hello(Hello),
    Welcome(Welcome),
    Request {
        #[serde(rename = "deadlineMs")]
        deadline_ms: u64,
        command: Command,
    },
    Chunk {
        data: Value,
    },
    Response {
        outcome: Outcome,
    },
    Cancel,
    Event {
        event: AgentEvent,
    },
}

/// Resultado final de um pedido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Ok { value: Value },
    Err { error: RemoteError },
}

impl From<Result<Value, RemoteError>> for Outcome {
    fn from(result: Result<Value, RemoteError>) -> Self {
        match result {
            Ok(value) => Self::Ok { value },
            Err(error) => Self::Err { error },
        }
    }
}

impl From<Outcome> for Result<Value, RemoteError> {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Ok { value } => Ok(value),
            Outcome::Err { error } => Err(error),
        }
    }
}

/// Primeiro quadro do agente: quem ele é e o que a política local permite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub protocol: u16,
    pub agent_version: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub in_container: bool,
    pub policy: Vec<Permission>,
    pub docker: Capability,
    pub compose: Capability,
}

/// Disponibilidade de um recurso no host do agente.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub available: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Welcome {
    pub probe_id: i64,
    pub name: String,
    /// 0 desliga os snapshots ao vivo; ver [`Command::SetLive`].
    pub live_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Docker {
        call: DockerCall,
    },
    Monitor {
        task: ProbeTask,
    },
    Discovery {
        task: ProbeDiscoveryTask,
    },
    /// Liga (> 0) ou desliga (0) os snapshots Docker ao vivo.
    SetLive {
        #[serde(rename = "intervalMs")]
        interval_ms: u64,
    },
    /// Uma operação de E/S de plugin de dispositivo (SSH exec, HTTP, Telnet)
    /// contra um equipamento da rede do agente. É uma operação unitária e
    /// fechada — o script do plugin roda na central, nunca aqui.
    DeviceIo {
        call: DeviceIoCall,
    },
    /// Ponte TCP até um banco da rede do agente (ADR 013). O agente só copia
    /// bytes — não fala SQL — e só alcança destinos da lista local
    /// `AGENT_DATABASE_TARGETS`. Depois do `Chunk` `{"opened": true}` os dados
    /// vão em [`TunnelFrame`]s com o id deste pedido.
    DatabaseTunnel {
        host: String,
        port: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum DockerCall {
    Status,
    ListContainers,
    InspectContainer {
        id: String,
    },
    ContainerAction {
        id: String,
        action: ContainerAction,
    },
    Logs {
        id: String,
        filters: LogFilters,
        #[serde(rename = "maxLines")]
        max_lines: usize,
    },
    /// Stream de linhas até `Cancel`.
    FollowLogs {
        id: String,
        tail: String,
    },
    ClearLogs {
        id: String,
    },
    Metrics,
    ListVolumes,
    InspectVolume {
        name: String,
    },
    RemoveVolume {
        name: String,
        force: bool,
    },
    ListNetworks,
    InspectNetwork {
        id: String,
    },
    CreateNetwork {
        name: String,
        driver: String,
    },
    RemoveNetwork {
        id: String,
    },
    ConnectNetwork {
        network: String,
        container: String,
    },
    DisconnectNetwork {
        network: String,
        container: String,
        force: bool,
    },
    ListImages,
    InspectImage {
        id: String,
    },
    RemoveImage {
        id: String,
        force: bool,
    },
    PruneImages,
    PullImage {
        image: String,
    },
    UpdateContainer {
        id: String,
    },
    Compose {
        request: ComposeRequest,
    },
}

impl Command {
    /// Permissão que a política do agente precisa conceder.
    #[must_use]
    pub const fn permission(&self) -> Permission {
        match self {
            Self::Docker { call } => call.permission(),
            Self::Monitor { .. } => Permission::Monitor,
            Self::Discovery { .. } => Permission::Discovery,
            Self::SetLive { .. } => Permission::Read,
            Self::DeviceIo { .. } => Permission::DeviceIo,
            Self::DatabaseTunnel { .. } => Permission::Database,
        }
    }
}

impl DockerCall {
    #[must_use]
    pub const fn permission(&self) -> Permission {
        match self {
            Self::Status
            | Self::ListContainers
            | Self::InspectContainer { .. }
            | Self::Logs { .. }
            | Self::FollowLogs { .. }
            | Self::Metrics
            | Self::ListVolumes
            | Self::InspectVolume { .. }
            | Self::ListNetworks
            | Self::InspectNetwork { .. }
            | Self::ListImages
            | Self::InspectImage { .. } => Permission::Read,
            Self::ContainerAction { .. }
            | Self::ClearLogs { .. }
            | Self::RemoveVolume { .. }
            | Self::CreateNetwork { .. }
            | Self::RemoveNetwork { .. }
            | Self::ConnectNetwork { .. }
            | Self::DisconnectNetwork { .. }
            | Self::RemoveImage { .. }
            | Self::PruneImages => Permission::Lifecycle,
            Self::PullImage { .. } | Self::UpdateContainer { .. } => Permission::Update,
            Self::Compose { .. } => Permission::Compose,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    DockerLive {
        snapshot: Box<DockerLiveSnapshot>,
    },
    DockerInventory {
        snapshot: Box<DockerInventorySnapshot>,
    },
    MetricsRollup {
        rollup: Box<MetricsRollup>,
    },
    /// Resultado de monitor que não conseguiu voltar como `Response` (o
    /// canal caiu no meio) e foi reenviado do buffer offline.
    MonitorResult {
        #[serde(rename = "monitorId")]
        monitor_id: i64,
        #[serde(rename = "taskId")]
        task_id: Option<String>,
        result: Box<CheckResult>,
    },
    DiscoveryResult {
        result: Box<ProbeDiscoveryResultPayload>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    Conflict,
    Validation,
    Unavailable,
    Disabled,
    Engine,
    Forbidden,
    Unsupported,
    Timeout,
    Disconnected,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteError {
    pub code: ErrorCode,
    pub message: String,
}

impl RemoteError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn disconnected() -> Self {
        Self::new(ErrorCode::Disconnected, "O agente se desconectou")
    }

    #[must_use]
    pub fn timeout() -> Self {
        Self::new(ErrorCode::Timeout, "O agente não respondeu a tempo")
    }
}

impl fmt::Display for RemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<DockerError> for RemoteError {
    fn from(error: DockerError) -> Self {
        let code = match &error {
            DockerError::Disabled => ErrorCode::Disabled,
            DockerError::Unavailable => ErrorCode::Unavailable,
            DockerError::NotFound => ErrorCode::NotFound,
            DockerError::Conflict => ErrorCode::Conflict,
            DockerError::Validation(_) => ErrorCode::Validation,
            DockerError::Engine => ErrorCode::Engine,
            DockerError::Forbidden(_) => ErrorCode::Forbidden,
            DockerError::Unsupported(_) => ErrorCode::Unsupported,
        };
        Self::new(code, error.to_string())
    }
}

/// O lado da central devolve os mesmos erros que a Engine local daria, para
/// que controller e tela não precisem saber que o host é remoto.
impl From<RemoteError> for DockerError {
    fn from(error: RemoteError) -> Self {
        match error.code {
            ErrorCode::Disabled => Self::Disabled,
            ErrorCode::Unavailable | ErrorCode::Timeout | ErrorCode::Disconnected => {
                Self::Unavailable
            }
            ErrorCode::NotFound => Self::NotFound,
            ErrorCode::Conflict => Self::Conflict,
            ErrorCode::Validation => Self::Validation(error.message),
            ErrorCode::Forbidden => Self::Forbidden(error.message),
            ErrorCode::Unsupported => Self::Unsupported(error.message),
            ErrorCode::Engine | ErrorCode::Internal => Self::Engine,
        }
    }
}

impl From<RemoteError> for AppError {
    fn from(error: RemoteError) -> Self {
        match error.code {
            ErrorCode::Timeout | ErrorCode::Disconnected | ErrorCode::Unavailable => {
                Self::service_unavailable(error.message)
            }
            ErrorCode::Forbidden => Self::forbidden(error.message),
            ErrorCode::NotFound => Self::not_found(error.message),
            ErrorCode::Validation => Self::validation(error.message),
            _ => DockerError::from(error).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn quadro_de_ponte_faz_ida_e_volta() {
        let id = Uuid::new_v4();
        for payload in [
            TunnelPayload::Data(b"SELECT 1".to_vec()),
            TunnelPayload::Data(Vec::new()),
            TunnelPayload::Ack(256 * 1024),
            TunnelPayload::Eof,
        ] {
            let frame = TunnelFrame { id, payload };
            assert_eq!(TunnelFrame::decode(&frame.encode()).unwrap(), frame);
        }
    }

    #[test]
    fn dado_comprimivel_vai_comprimido_e_volta_igual() {
        let id = Uuid::new_v4();
        let data = b"INSERT INTO clientes VALUES (1, 'Ana');\n".repeat(1000);
        let wire = TunnelFrame::encode_compressed(id, &data).expect("comprime");
        assert!(
            wire.len() < data.len() / 5,
            "{} de {}",
            wire.len(),
            data.len()
        );
        assert_eq!(
            TunnelFrame::decode(&wire).unwrap(),
            TunnelFrame {
                id,
                payload: TunnelPayload::Data(data),
            }
        );
    }

    #[test]
    fn dado_que_nao_comprime_nao_e_comprimido() {
        // Bytes pseudoaleatórios: o que um banco com TLS manda pela ponte.
        let mut state: u32 = 0x1234_5678;
        let noise: Vec<u8> = (0..32 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[0]
            })
            .collect();
        assert!(TunnelFrame::encode_compressed(Uuid::new_v4(), &noise).is_none());
    }

    #[test]
    fn quadro_comprimido_que_estoura_o_teto_e_recusado() {
        let id = Uuid::new_v4();
        let bomb = zstd::bulk::compress(&vec![0_u8; TUNNEL_DATA_MAX + 1], 1).unwrap();
        let mut wire = id.as_bytes().to_vec();
        wire.push(KIND_DATA_ZSTD);
        wire.extend_from_slice(&bomb);
        assert!(TunnelFrame::decode(&wire).is_err());
    }

    #[test]
    fn quadro_de_ponte_invalido_e_recusado() {
        let id = Uuid::new_v4();
        assert!(TunnelFrame::decode(&[0; 10]).is_err());
        let mut unknown = id.as_bytes().to_vec();
        unknown.push(9);
        assert!(TunnelFrame::decode(&unknown).is_err());
        let mut big = id.as_bytes().to_vec();
        big.push(0);
        big.extend(vec![0; TUNNEL_DATA_MAX + 1]);
        assert_eq!(TunnelFrame::decode(&big), Err(ProtocolError::FrameTooLarge));
    }

    #[test]
    fn ponte_de_banco_exige_permissao_propria() {
        let command = Command::DatabaseTunnel {
            host: "10.0.0.20".into(),
            port: 5432,
        };
        assert_eq!(command.permission(), Permission::Database);
        assert!(!crate::services::agents::policy::Policy::default().allows(Permission::Database));
    }

    #[test]
    fn envelope_faz_ida_e_volta() {
        let request = Envelope::new(Body::Request {
            deadline_ms: 5_000,
            command: Command::Docker {
                call: DockerCall::ContainerAction {
                    id: "web".into(),
                    action: ContainerAction::Remove { force: true },
                },
            },
        });
        let text = request.encode().expect("codifica");
        let again = Envelope::decode(&text).expect("decodifica");
        assert_eq!(again.encode().expect("recodifica"), text);

        let value: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value["body"]["kind"], "request");
        assert_eq!(value["body"]["command"]["op"], "docker");
        assert_eq!(value["body"]["command"]["call"]["call"], "container_action");
        assert_eq!(
            value["body"]["command"]["call"]["action"]["action"],
            "remove"
        );
    }

    #[test]
    fn resposta_carrega_erro_tipado() {
        let reply = Envelope::reply(
            Uuid::nil(),
            Body::Response {
                outcome: Err(RemoteError::from(DockerError::NotFound)).into(),
            },
        );
        let decoded = Envelope::decode(&reply.encode().expect("codifica")).expect("decodifica");
        let Body::Response { outcome } = decoded.body else {
            panic!("resposta");
        };
        let error = Result::<Value, RemoteError>::from(outcome).expect_err("erro");
        assert_eq!(error.code, ErrorCode::NotFound);
        assert!(matches!(DockerError::from(error), DockerError::NotFound));
    }

    #[test]
    fn quadro_grande_ou_invalido_e_recusado() {
        assert!(matches!(
            Envelope::decode(&"x".repeat(MAX_FRAME_BYTES + 1)),
            Err(ProtocolError::FrameTooLarge)
        ));
        assert!(matches!(
            Envelope::decode(r#"{"v":1}"#),
            Err(ProtocolError::Malformed(_))
        ));
    }

    #[test]
    fn cada_comando_exige_a_permissao_do_seu_risco() {
        let docker = |call| Command::Docker { call };
        assert_eq!(
            docker(DockerCall::ListContainers).permission(),
            Permission::Read
        );
        assert_eq!(
            docker(DockerCall::ContainerAction {
                id: "a".into(),
                action: ContainerAction::Stop
            })
            .permission(),
            Permission::Lifecycle
        );
        assert_eq!(
            docker(DockerCall::UpdateContainer { id: "a".into() }).permission(),
            Permission::Update
        );
        assert_eq!(
            docker(DockerCall::Compose {
                request: serde_json::from_value(json!({"project":"p","action":"up"}))
                    .expect("compose")
            })
            .permission(),
            Permission::Compose
        );
        assert_eq!(
            Command::SetLive { interval_ms: 0 }.permission(),
            Permission::Read
        );
    }
}
