//! DTOs de entrada dos recursos entregues nas Fases 2 e 3.

use chrono::{DateTime, Utc};
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

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInput {
    #[serde(default, deserialize_with = "deserialize_some")]
    pub site_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub network_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub parent_id: Option<Option<i64>>,
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
    /// `auto`, `local`, `vpn` ou `remote`.
    ///
    /// O `auto` é explícito de propósito, e não representado por ausência: a
    /// tela manda o formulário inteiro a cada gravação, e um campo ausente
    /// significa "não mexi" no resto deste DTO. Sem a palavra, voltar de uma
    /// declaração para o automático seria impossível pela interface.
    pub access_mode: Option<String>,
    /// `auto` ou um id do catálogo de `services::devices::systems`. O `auto` é
    /// explícito pelo mesmo motivo do `access_mode` logo acima.
    pub operating_system: Option<String>,
    pub link_interface_id: Option<i64>,
    pub link_interface_name: Option<String>,
    pub status: Option<String>,
    pub clear_history: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchParentInput {
    pub device_ids: Vec<i64>,
    pub parent_id: Option<i64>,
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
    /// Token cru do agente. O servidor calcula `sha256(token)` antes de gravar.
    /// Nunca envie `token_hash` — esse campo não existe mais no contrato.
    pub token: Option<String>,
    pub status: Option<String>,
    pub version: Option<String>,
    pub configuration: Option<serde_json::Value>,
}

/// Distingue "campo ausente" de "campo `null`" num payload parcial.
///
/// `#[serde(default)]` dá `None` quando a chave não veio; este desserializador
/// embrulha o valor lido em `Some`, de modo que `null` explícito chega como
/// `Some(None)`. É o único jeito de um `PUT` parcial poder **limpar** um
/// campo opcional em vez de só preenchê-lo.
fn presente<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Regra de alerta vinda da tela "Regras Configuradas".
///
/// Todo campo é opcional porque o `PUT` é parcial: o botão de ligar/desligar
/// da lista manda só `{ "enabled": false }`, e o restante da regra tem de
/// sobreviver. Campo ausente significa "não mexa", não "apague".
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleInput {
    // As três dimensões de escopo usam **dupla opção**, e a diferença importa:
    // `None` é "o cliente não mandou o campo" (o toggle da lista manda só
    // `enabled`) e `Some(None)` é "o cliente mandou `null`", que significa
    // *todos os dispositivos*. Com um `Option<i64>` simples as duas colapsam
    // em `None`, e o `input.device_id.or(current.device_id)` do `PUT` tornava
    // impossível devolver uma regra ao escopo global depois de vinculá-la.
    #[serde(default, deserialize_with = "presente")]
    pub site_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "presente")]
    pub device_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "presente")]
    pub monitor_id: Option<Option<i64>>,
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
    /// Intervalo mínimo entre notificações de problema do par (regra, alvo),
    /// mesmo quando o evento fecha e reabre (Fase 4). `0` desliga.
    pub notification_cooldown_seconds: Option<i32>,
    /// Suprimir a notificação quando o pai declarado do dispositivo já está em
    /// alerta (Fase 4).
    pub inhibit_when_parent_down: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogApplyInput {
    pub keys: Option<Vec<String>>,
    /// Escopo em que as regras nascem. Ausente = catálogo global, que é como
    /// `/alerts` se comporta antes de o operador escolher um dispositivo.
    pub site_id: Option<i64>,
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
}

/// `GET /api/alert-rules` e `GET /api/alert-rules/catalog` — recorte por escopo.
///
/// É o mesmo parâmetro nas duas rotas de propósito: abrir o catálogo pela
/// página do dispositivo já fixa aquele dispositivo, e listar as regras dele é
/// a mesma pergunta feita do outro lado.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleScopeQuery {
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
    pub site_id: Option<i64>,
    /// Junta ao recorte as regras **globais** (sem site, dispositivo nem
    /// monitor).
    ///
    /// Só faz sentido junto de `device_id`, e existe porque uma regra global
    /// criada de dentro de um equipamento sumia da tela em que nasceu — o que
    /// é indistinguível de a criação ter falhado. Fica opcional para o recorte
    /// continuar significando "só isto" onde é isso que se quer.
    pub include_global: Option<bool>,
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

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsBatchProvisionServerInput {
    pub server: String,
    pub name: Option<String>,
    pub protocol: Option<String>,
    pub doh_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsBatchProvisionInput {
    pub servers: Vec<DnsBatchProvisionServerInput>,
    pub domain: Option<String>,
    pub domains: Option<Vec<String>>,
    pub record_type: Option<String>,
    pub interval_seconds: Option<i32>,
    pub execute_now: Option<bool>,
    pub include_ping: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsPingProvisionInput {
    pub servers: Option<Vec<DnsBatchProvisionServerInput>>,
    pub interval_seconds: Option<i32>,
    pub execute_now: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryScanInput {
    pub network_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLinkInput {
    #[serde(alias = "source_device_id", alias = "source")]
    pub source_device_id: i64,
    #[serde(alias = "target_device_id", alias = "target")]
    pub target_device_id: i64,
    #[serde(alias = "source_interface_id")]
    pub source_interface_id: Option<i64>,
    #[serde(alias = "target_interface_id")]
    pub target_interface_id: Option<i64>,
    #[serde(alias = "link_type")]
    pub link_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLinkUpdateInput {
    #[serde(alias = "source_interface_id")]
    pub source_interface_id: Option<i64>,
    #[serde(alias = "target_interface_id")]
    pub target_interface_id: Option<i64>,
    #[serde(alias = "link_type")]
    pub link_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnmanagedSwitchInput {
    pub name: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    #[serde(default = "default_unmanaged_port_count")]
    pub port_count: u32,
    pub site_id: Option<i64>,
    pub network_id: Option<i64>,
}

fn default_unmanaged_port_count() -> u32 {
    8
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLayoutNodeInput {
    pub device_id: i64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLayoutInput {
    pub nodes: Vec<TopologyLayoutNodeInput>,
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

/// `GET /api/monitors/:id/uptime` — janela de horas para cálculo de uptime.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitorUptimeQuery {
    pub hours: Option<i64>,
}

/// Filtros suportados por `GET /api/monitors`
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitorsIndexQuery {
    pub enabled: Option<bool>,
    #[serde(alias = "isEnabled", alias = "is_enabled")]
    pub is_enabled: Option<bool>,
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub monitor_type: Option<String>,
    pub device_id: Option<i64>,
}

/// Janela de manutenção (Fase 3).
///
/// Toda janela precisa estar vinculada a um site ou a um dispositivo. A
/// validação semântica do intervalo fica no serviço de domínio.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceWindowInput {
    pub site_id: Option<i64>,
    pub device_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}
