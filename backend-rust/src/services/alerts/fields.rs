//! Vocabulário avaliável pelas regras de alerta (§8.7).
//!
//! Cada constante aqui é um `condition.field` válido. Os produtores de fatos
//! (os `datasets/`) só publicam chaves desta lista e o catálogo só constrói
//! condições a partir dela — assim a UI, o avaliador e os templates continuam
//! falando a mesma língua. Os rótulos em português vivem no frontend
//! (`frontend/src/utils/alertPresentation.ts`), que **espelha estas chaves**:
//! renomear qualquer uma aqui apaga o rótulo lá, sem erro de compilação em
//! nenhum dos dois lados.

// --- Resultado de monitor ---------------------------------------------------

/// Situação apurada na checagem: `up` | `down` | `warning` | `unknown`.
pub const STATUS: &str = "status";
pub const LATENCY_MS: &str = "latencyMs";
pub const PACKET_LOSS: &str = "packetLoss";
pub const STATUS_CODE: &str = "statusCode";
pub const DURATION_MS: &str = "durationMs";
pub const CONNECT_TIME_MS: &str = "connectTimeMs";
pub const RESOLUTION_TIME_MS: &str = "resolutionTimeMs";
/// Leitura SNMP pontual do monitor: 1 = up, 2 = down.
pub const IF_OPER_STATUS: &str = "ifOperStatus";
pub const IF_SPEED: &str = "ifSpeed";
pub const SNMP_UPTIME: &str = "snmpUptime";
pub const IN_BPS: &str = "inBps";
pub const OUT_BPS: &str = "outBps";

// --- Estado das interfaces coletadas via SNMP -------------------------------

/// Nome da interface — permite restringir a regra a uplinks, por exemplo.
pub const INTERFACE_NAME: &str = "interfaceName";
pub const INTERFACE_OPER_STATUS: &str = "interfaceOperStatus";
/// Transição observada no ciclo: `up_to_down` | `down_to_up`.
pub const INTERFACE_STATUS_TRANSITION: &str = "interfaceStatusTransition";
/// Velocidade negociada no ciclo atual, em bps.
pub const INTERFACE_SPEED_BPS: &str = "interfaceSpeedBps";
/// Renegociação observada no ciclo: `downgrade` | `upgrade`.
pub const INTERFACE_SPEED_TRANSITION: &str = "interfaceSpeedTransition";
/// Quanto a velocidade caiu, em % da anterior (apenas em downgrade).
pub const INTERFACE_SPEED_DROP_PERCENT: &str = "interfaceSpeedDropPercent";

/// Campos auxiliares publicados junto das transições, fora do vocabulário
/// oferecido na UI mas presentes no `data` do alerta para o operador entender
/// o que mudou.
pub const INTERFACE_PREVIOUS_OPER_STATUS: &str = "interfacePreviousOperStatus";
pub const INTERFACE_PREVIOUS_SPEED_BPS: &str = "interfacePreviousSpeedBps";

// --- Túneis WireGuard -------------------------------------------------------

/// Nome do equipamento do outro lado do túnel.
pub const VPN_PEER_NAME: &str = "vpnPeerName";
/// Estado atual do túnel: `connected` | `unstable` | `disconnected` | `awaiting`.
pub const VPN_PEER_STATUS: &str = "vpnPeerStatus";
/// Transição observada no ciclo (ver [`vpn_status_transition`]).
pub const VPN_STATUS_TRANSITION: &str = "vpnStatusTransition";
/// Segundos desde o último sinal de vida (keepalive ou handshake).
pub const VPN_SECONDS_SINCE_ACTIVITY: &str = "vpnSecondsSinceActivity";
pub const VPN_PREVIOUS_STATUS: &str = "vpnPreviousStatus";

/// Vocabulário completo oferecido na tela de regras, na ordem em que o
/// AdonisJS o declarava.
pub const ALERT_FIELDS: [&str; 24] = [
    STATUS,
    LATENCY_MS,
    PACKET_LOSS,
    STATUS_CODE,
    DURATION_MS,
    CONNECT_TIME_MS,
    RESOLUTION_TIME_MS,
    IF_OPER_STATUS,
    IF_SPEED,
    SNMP_UPTIME,
    IN_BPS,
    OUT_BPS,
    INTERFACE_NAME,
    INTERFACE_OPER_STATUS,
    INTERFACE_STATUS_TRANSITION,
    INTERFACE_SPEED_BPS,
    INTERFACE_SPEED_TRANSITION,
    INTERFACE_SPEED_DROP_PERCENT,
    VPN_PEER_NAME,
    VPN_PEER_STATUS,
    VPN_STATUS_TRANSITION,
    VPN_SECONDS_SINCE_ACTIVITY,
    // Fora da tela, mas avaliáveis: publicados pelos datasets.
    "success",
    "type",
];

/// Valores de [`INTERFACE_STATUS_TRANSITION`].
pub mod interface_status_transition {
    pub const WENT_DOWN: &str = "up_to_down";
    pub const CAME_BACK: &str = "down_to_up";
}

/// Valores de [`INTERFACE_SPEED_TRANSITION`].
pub mod interface_speed_transition {
    pub const DOWNGRADE: &str = "downgrade";
    pub const UPGRADE: &str = "upgrade";
}

/// Transições de túnel que valem como fato alertável.
///
/// `awaiting` (peer criado e nunca conectado) não entra: não houve queda, o
/// túnel simplesmente ainda não subiu — alertar aí seria alarme de instalação.
pub mod vpn_status_transition {
    pub const DISCONNECTED: &str = "connected_to_disconnected";
    pub const DESTABILIZED: &str = "connected_to_unstable";
    pub const RECONNECTED: &str = "reconnected";
}
