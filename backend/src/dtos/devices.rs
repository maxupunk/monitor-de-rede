//! A projeção de capacidades de um dispositivo, tipada para o frontend.
//!
//! O struct mora aqui, e não no serviço, pelo mesmo motivo dos demais DTOs: é
//! contrato HTTP, e contrato de fronteira é exportado por `ts-rs` para que a
//! tela não invente campo nem deduza suporte a partir de nome ou de ID. Quem o
//! **calcula** é `services::devices::capabilities`, a partir de evidência
//! persistida.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// O que a página de detalhe pode mostrar e oferecer.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeviceCapabilities {
    #[ts(type = "number")]
    pub device_id: i64,
    /// Este é o dispositivo que representa a instalação.
    pub is_system: bool,

    // --- Abas ---
    /// SNMP declarado no cadastro. Intenção, não prova.
    pub snmp_configured: bool,
    /// Houve comunicação SNMP bem-sucedida e persistida.
    pub snmp_connected: bool,
    /// Há inventário de interfaces para listar.
    pub interfaces: bool,
    /// Há histórico de eventos/alertas.
    pub events: bool,
    /// Log ativo, suportado ou já recebido.
    pub logs: bool,
    /// O dispositivo é a ponta de um túnel.
    pub vpn: bool,
    /// O dispositivo publica métricas de saúde (CPU, memória, …).
    pub health: bool,

    // --- Ações do cabeçalho ---
    /// Varredura SNMP (descobrir interfaces e sistema).
    pub can_snmp_scan: bool,
    /// Coleta SNMP pontual.
    pub can_snmp_collect: bool,
    /// Escanear portas do alvo.
    pub can_scan_ports: bool,
    /// Editar identidade (IP, tipo, SNMP).
    pub can_edit_identity: bool,
    /// Criar monitores próprios.
    pub can_create_monitor: bool,

    /// Por que este dispositivo não recebe um monitor de alcance automático
    /// (ping/TCP/HTTP/DNS), em português. `None` quando recebe.
    ///
    /// A tela mostra este texto em vez de deduzir o motivo: são duas causas
    /// diferentes — ser o dispositivo do sistema ou não ter endereço IP — e
    /// cada uma pede uma ação diferente do operador.
    pub reach_monitor_blocked_reason: Option<String>,

    /// O vocabulário de alerta que este dispositivo publica hoje.
    ///
    /// É sobre esta lista que a aplicabilidade de um template é decidida: um
    /// template de CPU só é oferecido a quem publica `cpuUsagePercent`.
    pub alert_fields: Vec<String>,
}

/// Referência compacta a um site.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SiteRef {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub name: String,
}

/// Referência compacta a um dispositivo pai.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ParentRef {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub name: String,
}

/// Projeção canônica completa de um dispositivo para a API HTTP.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DevicePresenterItem {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub site_id: Option<i64>,
    #[ts(type = "number | null")]
    pub network_id: Option<i64>,
    #[ts(type = "number | null")]
    pub parent_id: Option<i64>,
    pub ip_address: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub description: Option<String>,
    pub is_monitored: bool,
    pub snmp_enabled: bool,
    pub snmp_community: Option<String>,
    pub snmp_version: Option<String>,
    pub snmp_poll_interval_seconds: i32,
    pub status: String,
    pub access_mode: Option<String>,
    pub effective_access_mode: String,
    pub access_mode_reason: String,
    pub access_mode_declared: bool,
    pub operating_system: Option<String>,
    pub effective_operating_system: String,
    pub operating_system_source: String,
    pub operating_system_reason: String,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub site: Option<SiteRef>,
    pub parent: Option<ParentRef>,
    pub system_key: Option<String>,
    pub is_system: bool,
    #[ts(type = "number | null")]
    pub link_interface_id: Option<i64>,
    pub link_interface_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "any")]
    pub vpn_peer: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_metric_item_serializa_em_camel_case() {
        let item = DeviceMetricItem {
            id: 1,
            device_id: 2,
            interface_id: Some(3),
            interface_name: Some("eth0".into()),
            metric_name: "inBps".into(),
            metric_value: 1_000.0,
            unit: "bps".into(),
            created_at: "01/01/2026 12:00:00".into(),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["deviceId"], 2);
        assert_eq!(json["interfaceId"], 3);
        assert_eq!(json["metricName"], "inBps");
        assert_eq!(json["metricValue"], 1_000.0);
    }

    #[test]
    fn device_event_item_serializa_em_camel_case() {
        let item = DeviceEventItem {
            id: 1,
            device_id: 2,
            event_type: "down".into(),
            severity: "critical".into(),
            message: "caiu".into(),
            created_at: "01/01/2026 12:00:00".into(),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["eventType"], "down");
        assert_eq!(json["deviceId"], 2);
    }
}

/// Métrica de um dispositivo exibida na aba de métricas.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeviceMetricItem {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub device_id: i64,
    #[ts(type = "number | null")]
    pub interface_id: Option<i64>,
    pub interface_name: Option<String>,
    pub metric_name: String,
    #[ts(type = "number")]
    pub metric_value: f64,
    pub unit: String,
    pub created_at: String,
}

/// Evento de alerta de um dispositivo exibido na aba de eventos.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeviceEventItem {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub device_id: i64,
    pub event_type: String,
    pub severity: String,
    pub message: String,
    pub created_at: String,
}
