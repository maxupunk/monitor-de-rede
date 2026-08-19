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
