//! Contrato de dados do teste de enlace (ADR 015 §5.1), exportado por `ts-rs`.
//!
//! `forward` é sempre o sentido **A→B** da perna; `reverse`, B→A. Os nomes
//! seguem o ADR ao pé da letra: é o mesmo contrato que a tela consome.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::{
    network_tools::throughput::{
        ThroughputEngine, MAX_DURATION_S, MAX_STREAMS, MIN_DURATION_S, MIN_STREAMS,
    },
    plugins::manifest::TransportKind,
    shared::errors::{AppError, AppResult},
};

/// Duração padrão de cada sentido, em segundos.
pub const DEFAULT_DURATION_S: u8 = 5;
/// Menor intervalo de um agendamento, em minutos.
pub const MIN_INTERVAL_MINUTES: i32 = 15;

/// O que se mede.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestKind {
    /// Um enlace (dois dispositivos).
    #[default]
    Link,
    /// O caminho da origem ao destino, um enlace por vez.
    Path,
    /// Do servidor (ou do agente do site) até o destino.
    Server,
}

/// Estado de um teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestStatus {
    Queued,
    Running,
    Succeeded,
    Partial,
    Failed,
    Cancelled,
}

impl LinkTestStatus {
    /// O teste ainda pode mudar.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

/// Estado de uma perna ou de uma etapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestStepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

impl LinkTestStepStatus {
    /// A etapa já terminou (de qualquer jeito).
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Skipped | Self::Cancelled
        )
    }
}

/// Sentido de uma medida, no enlace: `forward` = A→B, `reverse` = B→A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestFlow {
    Forward,
    Reverse,
}

/// Quais sentidos medir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestDirection {
    #[default]
    Both,
    Forward,
    Reverse,
}

impl LinkTestDirection {
    /// Os sentidos pedidos, na ordem em que rodam.
    #[must_use]
    pub fn flows(self) -> Vec<LinkTestFlow> {
        match self {
            Self::Both => vec![LinkTestFlow::Forward, LinkTestFlow::Reverse],
            Self::Forward => vec![LinkTestFlow::Forward],
            Self::Reverse => vec![LinkTestFlow::Reverse],
        }
    }
}

/// Especificação pedida — tudo opcional, com os padrões do ADR.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestSpecInput {
    /// Duração de cada sentido (padrão 5; 1–60).
    #[serde(default)]
    #[ts(optional)]
    pub duration_s: Option<u8>,
    #[serde(default)]
    #[ts(optional)]
    pub direction: Option<LinkTestDirection>,
    /// Fluxos TCP paralelos (padrão 1; 1–8, só iperf3).
    #[serde(default)]
    #[ts(optional)]
    pub streams: Option<u8>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub limit_mbps: Option<u32>,
}

/// Especificação completa (`Required<LinkTestSpecInput>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestSpec {
    pub duration_s: u8,
    pub direction: LinkTestDirection,
    pub streams: u8,
    pub limit_mbps: Option<u32>,
}

impl Default for LinkTestSpec {
    fn default() -> Self {
        Self {
            duration_s: DEFAULT_DURATION_S,
            direction: LinkTestDirection::Both,
            streams: 1,
            limit_mbps: None,
        }
    }
}

impl LinkTestSpecInput {
    /// Aplica os padrões e confere os limites do contrato.
    ///
    /// # Errors
    ///
    /// [`AppError::Validation`] com o primeiro campo fora da faixa.
    pub fn resolve(&self) -> AppResult<LinkTestSpec> {
        let defaults = LinkTestSpec::default();
        let spec = LinkTestSpec {
            duration_s: self.duration_s.unwrap_or(defaults.duration_s),
            direction: self.direction.unwrap_or(defaults.direction),
            streams: self.streams.unwrap_or(defaults.streams),
            limit_mbps: self.limit_mbps,
        };
        if !(MIN_DURATION_S..=MAX_DURATION_S).contains(&spec.duration_s) {
            return Err(AppError::validation(format!(
                "A duração do teste deve ficar entre {MIN_DURATION_S} e {MAX_DURATION_S} segundos"
            )));
        }
        if !(MIN_STREAMS..=MAX_STREAMS).contains(&spec.streams) {
            return Err(AppError::validation(format!(
                "O número de fluxos deve ficar entre {MIN_STREAMS} e {MAX_STREAMS}"
            )));
        }
        if spec.limit_mbps == Some(0) {
            return Err(AppError::validation(
                "O limite de banda deve ser maior que zero (omita para não limitar)",
            ));
        }
        Ok(spec)
    }
}

/// Pedido de plano ou de execução.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestRequest {
    pub kind: LinkTestKind,
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub link_id: Option<i64>,
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub source_device_id: Option<i64>,
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub target_device_id: Option<i64>,
    #[serde(default)]
    #[ts(optional)]
    pub spec: Option<LinkTestSpecInput>,
    /// Obrigatório `true` para rodar: medir satura o enlace.
    #[serde(default)]
    #[ts(optional)]
    pub confirm: Option<bool>,
}

/// Quem é uma ponta da perna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestEndpointKind {
    /// Um equipamento, pelo plugin com papéis de teste.
    Device,
    /// O próprio servidor NetMonitor (motores nativos).
    Central,
    /// Um agente remoto (só dirige).
    Agent,
}

/// Uma ponta da perna, como a tela a mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestEndpoint {
    pub kind: LinkTestEndpointKind,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    #[ts(type = "number | null")]
    pub agent_id: Option<i64>,
    pub name: String,
    pub ip: Option<String>,
    pub platform: Option<String>,
}

/// Pacote que o plano instala num equipamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestInstall {
    #[ts(type = "number")]
    pub device_id: i64,
    pub device_name: String,
    pub packages: Vec<String>,
}

/// Um passo que o operador faz ali mesmo para a perna poder rodar. `message`
/// é a frase que também vai em `issues` — a tela a troca pelo botão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestFix {
    /// Instalar (e ligar, se desligado) o plugin de teste no equipamento.
    #[serde(rename_all = "camelCase")]
    InstallPlugin {
        #[ts(type = "number")]
        device_id: i64,
        device_name: String,
        #[ts(type = "number")]
        plugin_id: i64,
        plugin_slug: String,
        plugin_name: String,
        message: String,
    },
    /// Cadastrar (ou completar) o acesso do equipamento por um transporte.
    #[serde(rename_all = "camelCase")]
    AddCredential {
        #[ts(type = "number")]
        device_id: i64,
        device_name: String,
        transport: TransportKind,
        message: String,
    },
}

impl LinkTestFix {
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::InstallPlugin { message, .. } | Self::AddCredential { message, .. } => message,
        }
    }
}

/// Tipo de perna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestLegKind {
    /// Um enlace da topologia.
    Link,
    /// Do servidor (ou agente) direto ao destino.
    EndToEnd,
}

/// Lado da perna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestSide {
    A,
    B,
}

impl LinkTestSide {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
        }
    }
}

/// Uma perna do plano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestPlanLeg {
    pub seq: u32,
    pub kind: LinkTestLegKind,
    pub a: LinkTestEndpoint,
    pub b: LinkTestEndpoint,
    #[ts(type = "number | null")]
    pub edge_id: Option<i64>,
    pub engine: Option<ThroughputEngine>,
    pub server_side: Option<LinkTestSide>,
    pub duration_s: u8,
    pub runnable: bool,
    pub issues: Vec<String>,
    /// Os passos que resolvem parte de `issues` ali mesmo (instalar o
    /// plugin, cadastrar o acesso). Só no plano.
    #[serde(default)]
    pub fixes: Vec<LinkTestFix>,
}

/// O plano antes de rodar: pernas, motores, o que será instalado, o que
/// falta e quanto tempo leva.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestPlan {
    pub kind: LinkTestKind,
    pub legs: Vec<LinkTestPlanLeg>,
    pub estimated_seconds: u32,
    pub warnings: Vec<String>,
    /// O que impede de rodar e não se resolve com um dos `fixes`.
    pub blocking: Vec<String>,
    pub will_install: Vec<LinkTestInstall>,
    /// Os passos que faltam, sem repetição. Enquanto nenhuma perna roda, eles
    /// também impedem o teste: roda quando `blocking` está vazio e há perna
    /// executável.
    pub fixes: Vec<LinkTestFix>,
}

/// Etapa de uma perna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestStageKey {
    PrepareA,
    PrepareB,
    Serve,
    Forward,
    Reverse,
    Stop,
}

impl LinkTestStageKey {
    /// Todas, na ordem em que rodam.
    pub const ALL: [Self; 6] = [
        Self::PrepareA,
        Self::PrepareB,
        Self::Serve,
        Self::Forward,
        Self::Reverse,
        Self::Stop,
    ];

    /// Rótulo da tela.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PrepareA => "Preparar A",
            Self::PrepareB => "Preparar B",
            Self::Serve => "Subir servidor",
            Self::Forward => "A → B",
            Self::Reverse => "B → A",
            Self::Stop => "Encerrar",
        }
    }

    /// A etapa de medida de um sentido.
    #[must_use]
    pub const fn of_flow(flow: LinkTestFlow) -> Self {
        match flow {
            LinkTestFlow::Forward => Self::Forward,
            LinkTestFlow::Reverse => Self::Reverse,
        }
    }

    /// A etapa de preparo de um lado.
    #[must_use]
    pub const fn prepare_of(side: LinkTestSide) -> Self {
        match side {
            LinkTestSide::A => Self::PrepareA,
            LinkTestSide::B => Self::PrepareB,
        }
    }
}

/// Uma etapa, com `runId` do plugin quando houver (transcript a um clique).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestStage {
    pub key: LinkTestStageKey,
    pub label: String,
    pub status: LinkTestStepStatus,
    pub message: Option<String>,
    #[ts(type = "number | null")]
    pub run_id: Option<i64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl LinkTestStage {
    /// Etapa ainda não iniciada.
    #[must_use]
    pub fn pending(key: LinkTestStageKey) -> Self {
        Self {
            key,
            label: key.label().to_owned(),
            status: LinkTestStepStatus::Pending,
            message: None,
            run_id: None,
            started_at: None,
            finished_at: None,
        }
    }

    /// As seis etapas de uma perna, pendentes.
    #[must_use]
    pub fn all_pending() -> Vec<Self> {
        LinkTestStageKey::ALL
            .into_iter()
            .map(Self::pending)
            .collect()
    }
}

/// Uma amostra de vazão, no sentido do enlace.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestSample {
    pub flow: LinkTestFlow,
    pub bps: f64,
    #[ts(type = "number")]
    pub t_ms: u64,
}

/// Uma perna com o andamento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestLeg {
    #[serde(flatten)]
    #[ts(flatten)]
    pub plan: LinkTestPlanLeg,
    pub status: LinkTestStepStatus,
    pub stages: Vec<LinkTestStage>,
    pub forward_bps: Option<f64>,
    pub reverse_bps: Option<f64>,
    #[ts(type = "number | null")]
    pub retransmits: Option<i64>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// Só no `GET` de detalhe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub samples: Option<Vec<LinkTestSample>>,
}

/// Resumo: o gargalo é a perna com a menor vazão entre as que mediram.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestSummary {
    pub bottleneck_leg_seq: Option<u32>,
    pub min_forward_bps: Option<f64>,
    pub min_reverse_bps: Option<f64>,
}

/// Quem pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum LinkTestOrigin {
    User,
    Schedule,
}

/// Referência a um usuário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestUserRef {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
}

/// Um teste, completo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTest {
    #[ts(type = "number")]
    pub id: i64,
    pub kind: LinkTestKind,
    pub status: LinkTestStatus,
    pub origin: LinkTestOrigin,
    #[ts(type = "number | null")]
    pub schedule_id: Option<i64>,
    #[ts(type = "number | null")]
    pub link_id: Option<i64>,
    #[ts(type = "number | null")]
    pub source_device_id: Option<i64>,
    #[ts(type = "number | null")]
    pub target_device_id: Option<i64>,
    pub spec: LinkTestSpec,
    pub legs: Vec<LinkTestLeg>,
    pub summary: Option<LinkTestSummary>,
    pub error: Option<String>,
    pub requested_by: Option<LinkTestUserRef>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

/// Uma página da lista.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestPage {
    pub items: Vec<LinkTest>,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub page: u64,
    #[ts(type = "number")]
    pub per_page: u64,
}

/// Filtros da lista (`?deviceId=&linkId=&status=&kind=&since=&page=&perPage=`).
/// `since` é RFC 3339: testes criados a partir dele.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTestQuery {
    #[serde(default)]
    pub kind: Option<LinkTestKind>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub device_id: Option<i64>,
    #[serde(default)]
    pub link_id: Option<i64>,
    #[serde(default)]
    pub status: Option<LinkTestStatus>,
    #[serde(default)]
    pub page: Option<u64>,
    #[serde(default)]
    pub per_page: Option<u64>,
}

/// Resposta do `POST /api/link-tests` (202).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestStarted {
    #[ts(type = "number")]
    pub id: i64,
}

/// Um agendamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestSchedule {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub kind: LinkTestKind,
    #[ts(type = "number | null")]
    pub link_id: Option<i64>,
    #[ts(type = "number | null")]
    pub source_device_id: Option<i64>,
    #[ts(type = "number | null")]
    pub target_device_id: Option<i64>,
    pub spec: LinkTestSpec,
    /// ≥ 15.
    pub interval_minutes: i32,
    /// `"HH:MM"`, hora local do servidor.
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    #[ts(type = "number | null")]
    pub last_test_id: Option<i64>,
    pub last_status: Option<LinkTestStatus>,
    pub created_at: String,
    pub updated_at: String,
}

/// Criação/edição de um agendamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestScheduleInput {
    pub name: String,
    pub kind: LinkTestKind,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub link_id: Option<i64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub source_device_id: Option<i64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub target_device_id: Option<i64>,
    pub interval_minutes: i32,
    #[serde(default)]
    pub window_start: Option<String>,
    #[serde(default)]
    pub window_end: Option<String>,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    #[ts(optional)]
    pub spec: Option<LinkTestSpecInput>,
}

const fn enabled_by_default() -> bool {
    true
}

/// Evento efêmero `link_test:sample`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestSampleEvent {
    #[ts(type = "number")]
    pub test_id: i64,
    pub leg_seq: u32,
    pub flow: LinkTestFlow,
    pub bps: f64,
    #[ts(type = "number")]
    pub t_ms: u64,
}

/// Evento `link_test_schedule:updated` — a tela relê a lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestScheduleEvent {
    #[ts(type = "number")]
    pub id: i64,
}

/// Texto de um enum serializado em `snake_case` (o que vai ao banco).
#[must_use]
pub fn as_db<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// O enum a partir do texto gravado.
#[must_use]
pub fn from_db<T: for<'de> Deserialize<'de>>(value: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn spec_aplica_padroes_e_limites() {
        let spec = LinkTestSpecInput::default().resolve().unwrap();
        assert_eq!(spec, LinkTestSpec::default());
        assert_eq!(spec.duration_s, 5);
        assert_eq!(spec.direction, LinkTestDirection::Both);
        for ruim in [
            LinkTestSpecInput {
                duration_s: Some(0),
                ..Default::default()
            },
            LinkTestSpecInput {
                duration_s: Some(61),
                ..Default::default()
            },
            LinkTestSpecInput {
                streams: Some(9),
                ..Default::default()
            },
            LinkTestSpecInput {
                limit_mbps: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(ruim.resolve(), Err(AppError::Validation(_))));
        }
    }

    #[test]
    fn contrato_em_camel_case() {
        let pedido: LinkTestRequest = serde_json::from_value(json!({
            "kind": "path", "sourceDeviceId": 1, "targetDeviceId": 2,
            "spec": { "durationS": 10, "direction": "forward", "limitMbps": null },
            "confirm": true
        }))
        .unwrap();
        assert_eq!(pedido.kind, LinkTestKind::Path);
        assert_eq!(pedido.spec.unwrap().duration_s, Some(10));

        let perna = LinkTestLeg {
            plan: LinkTestPlanLeg {
                seq: 1,
                kind: LinkTestLegKind::EndToEnd,
                a: LinkTestEndpoint {
                    kind: LinkTestEndpointKind::Central,
                    device_id: Some(1),
                    agent_id: None,
                    name: "Servidor".into(),
                    ip: None,
                    platform: None,
                },
                b: LinkTestEndpoint {
                    kind: LinkTestEndpointKind::Device,
                    device_id: Some(2),
                    agent_id: None,
                    name: "R1".into(),
                    ip: Some("10.0.0.1".into()),
                    platform: Some("routeros".into()),
                },
                edge_id: None,
                engine: Some(ThroughputEngine::Btest),
                server_side: Some(LinkTestSide::A),
                duration_s: 5,
                runnable: true,
                issues: vec![],
                fixes: vec![],
            },
            status: LinkTestStepStatus::Pending,
            stages: LinkTestStage::all_pending(),
            forward_bps: None,
            reverse_bps: None,
            retransmits: None,
            error: None,
            started_at: None,
            finished_at: None,
            samples: None,
        };
        let valor = serde_json::to_value(&perna).unwrap();
        assert_eq!(valor["kind"], "end_to_end");
        assert_eq!(valor["serverSide"], "a");
        assert_eq!(valor["a"]["kind"], "central");
        assert_eq!(valor["stages"][0]["key"], "prepare_a");
        assert_eq!(valor["stages"][5]["label"], "Encerrar");
        assert!(valor.get("samples").is_none(), "só no detalhe");
        let volta: LinkTestLeg = serde_json::from_value(valor).unwrap();
        assert_eq!(volta, perna);

        let passo = LinkTestFix::AddCredential {
            device_id: 2,
            device_name: "R1".into(),
            transport: TransportKind::Ssh,
            message: "R1 está sem acesso SSH".into(),
        };
        assert_eq!(
            serde_json::to_value(&passo).unwrap(),
            json!({ "kind": "add_credential", "deviceId": 2, "deviceName": "R1",
                    "transport": "ssh", "message": "R1 está sem acesso SSH" })
        );
    }

    #[test]
    fn enums_vao_ao_banco_como_texto() {
        assert_eq!(as_db(&LinkTestStatus::Partial), "partial");
        assert_eq!(as_db(&LinkTestLegKind::EndToEnd), "end_to_end");
        assert_eq!(
            from_db::<LinkTestStatus>("cancelled"),
            Some(LinkTestStatus::Cancelled)
        );
        assert_eq!(from_db::<LinkTestStatus>("x"), None);
        assert!(LinkTestStatus::Running.is_active());
        assert!(!LinkTestStatus::Failed.is_active());
        assert_eq!(
            LinkTestDirection::Reverse.flows(),
            vec![LinkTestFlow::Reverse]
        );
    }
}
