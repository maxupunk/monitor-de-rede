//! DTOs de entrada dos recursos entregues nas Fases 2 e 3.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteInput {
    pub name: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInput {
    pub site_id: Option<i64>,
    pub probe_id: Option<i64>,
    pub name: String,
    pub cidr: String,
    pub gateway: Option<String>,
    pub vlan: Option<i32>,
    pub dns_servers: Option<serde_json::Value>,
    pub scan_enabled: Option<bool>,
    pub scan_interval: Option<i32>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInput {
    pub site_id: Option<i64>,
    pub network_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub ip_address: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub description: Option<String>,
    pub is_monitored: Option<bool>,
    pub snmp_enabled: Option<bool>,
    pub snmp_community: Option<String>,
    pub snmp_version: Option<String>,
    pub snmp_poll_interval_seconds: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInput {
    pub device_id: Option<i64>,
    pub probe_id: Option<i64>,
    #[serde(rename = "type")]
    pub monitor_type: Option<String>,
    pub name: Option<String>,
    pub configuration: Option<serde_json::Value>,
    pub target: Option<String>,
    pub port: Option<i64>,
    pub interval_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub enabled: Option<bool>,
    pub is_enabled: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProbeInput {
    pub site_id: Option<i64>,
    pub name: Option<String>,
    pub token_hash: Option<String>,
    pub status: Option<String>,
    pub version: Option<String>,
    pub configuration: Option<serde_json::Value>,
}

/// Regra de alerta vinda da tela "Regras Configuradas".
///
/// Todo campo é opcional porque o `PUT` é parcial: o botão de ligar/desligar
/// da lista manda só `{ "enabled": false }`, e o restante da regra tem de
/// sobreviver. Campo ausente significa "não mexa", não "apague".
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleInput {
    pub site_id: Option<i64>,
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub rule_type: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub severity: Option<String>,
    pub duration_seconds: Option<i32>,
    /// Janela de estabilidade antes de resolver (Fase 1 do roadmap de alertas
    /// inteligentes). `None` no PUT = mantém; `0` = resolve na primeira ok.
    pub recovery_window_seconds: Option<i32>,
    /// Recaídas dentro da janela que declaram o alvo oscilando (Fase 3).
    /// `None` no PUT = mantém; `0` = detecção desligada.
    pub flap_threshold: Option<i32>,
    /// Largura da janela deslizante da detecção de flapping (Fase 3).
    pub flap_window_seconds: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogApplyInput {
    pub keys: Option<Vec<String>>,
}

/// `GET /api/alerts/instability` — histórico de oscilação por alvo (Fase 3).
///
/// Sem `scopeKey` devolve o ranking de todos os alvos; com ele, só o alvo
/// pedido (é como a página do monitor consulta o próprio indicador).
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstabilityQuery {
    pub hours: Option<i64>,
    pub scope_key: Option<String>,
}

/// `POST /api/alerts/:id/silence`. O frontend manda `minutes`; `durationMinutes`
/// é aceito por compatibilidade com integrações antigas, que ainda o enviam.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SilenceInput {
    pub minutes: Option<i64>,
    pub duration_minutes: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsServerInput {
    pub name: Option<String>,
    pub address: Option<String>,
    pub protocol: Option<String>,
    pub is_default: Option<bool>,
    pub description: Option<String>,
}

/// Payload da ferramenta de varredura. A validação semântica fica no
/// controller para que a mensagem continue compatível com o frontend.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortScanInput {
    pub host: String,
    pub protocol: String,
    pub ports: Vec<u16>,
    pub timeout_ms: Option<u64>,
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsLookupInput {
    pub hostname: String,
    pub server: Option<String>,
    pub protocol: Option<String>,
    pub doh_url: Option<String>,
    pub record_type: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsBenchmarkServerInput {
    pub server: String,
    pub label: Option<String>,
    pub protocol: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsBenchmarkInput {
    pub servers: Option<Vec<DnsBenchmarkServerInput>>,
    pub hostnames: Option<Vec<String>>,
    pub record_type: Option<String>,
    pub timeout_ms: Option<u64>,
    pub rounds: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryScanInput {
    pub network_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLinkInput {
    #[serde(alias = "source_device_id")]
    pub source_device_id: i64,
    #[serde(alias = "target_device_id")]
    pub target_device_id: i64,
    #[serde(alias = "source_interface_id")]
    pub source_interface_id: Option<i64>,
    #[serde(alias = "target_interface_id")]
    pub target_interface_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardLayoutInput {
    pub layout: Vec<serde_json::Value>,
    pub client_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}
