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

// --- Baseline móvel e anomalias estatísticas (§2.3.3) -----------------------

/// Média histórica de latência (ms) na janela de baseline.
pub const LATENCY_BASELINE_MS: &str = "latencyBaselineMs";
/// Desvio padrão da latência (ms) na janela de baseline.
pub const LATENCY_STDDEV_MS: &str = "latencyStddevMs";
/// Desvio percentual da latência atual em relação à baseline.
pub const LATENCY_DEVIATION_PERCENT: &str = "latencyDeviationPercent";
/// Z-Score da latência (desvios padrão acima da média histórica).
pub const LATENCY_Z_SCORE: &str = "latencyZScore";
/// Limiar da banda superior esperada de latência (média + 3 sigmas).
pub const LATENCY_UPPER_BAND_MS: &str = "latencyUpperBandMs";

/// Média histórica de perda de pacotes (%) na janela de baseline.
pub const PACKET_LOSS_BASELINE_PERCENT: &str = "packetLossBaselinePercent";
/// Desvio padrão da perda de pacotes (%) na janela de baseline.
pub const PACKET_LOSS_STDDEV_PERCENT: &str = "packetLossStddevPercent";
/// Diferença absoluta da perda atual em relação à baseline (p.p.).
pub const PACKET_LOSS_DEVIATION_PERCENT: &str = "packetLossDeviationPercent";
/// Z-Score da perda de pacotes (desvios padrão acima da média histórica).
pub const PACKET_LOSS_Z_SCORE: &str = "packetLossZScore";
/// Limiar da banda superior esperada de perda de pacotes (média + 3 sigmas).
pub const PACKET_LOSS_UPPER_BAND_PERCENT: &str = "packetLossUpperBandPercent";

/// Média histórica de uptime (%) na janela de baseline.
pub const UPTIME_BASELINE_PERCENT: &str = "uptimeBaselinePercent";
/// Desvio padrão do uptime (%) na janela de baseline.
pub const UPTIME_STDDEV_PERCENT: &str = "uptimeStddevPercent";
/// Quanto o uptime atual caiu abaixo da baseline (p.p.).
pub const UPTIME_DEVIATION_PERCENT: &str = "uptimeDeviationPercent";
/// Z-Score de queda de uptime (desvios padrão abaixo da média histórica).
pub const UPTIME_Z_SCORE: &str = "uptimeZScore";

/// Média histórica do volume de logs syslog na janela.
pub const SYSLOG_VOLUME_BASELINE: &str = "syslogVolumeBaseline";
/// Desvio padrão do volume de logs syslog na janela.
pub const SYSLOG_VOLUME_STDDEV: &str = "syslogVolumeStddev";
/// Z-Score do volume de logs syslog.
pub const SYSLOG_VOLUME_Z_SCORE: &str = "syslogVolumeZScore";

/// Z-Score do tráfego de entrada na interface.
pub const TRAFFIC_IN_Z_SCORE: &str = "trafficInZScore";
/// Z-Score do tráfego de saída na interface.
pub const TRAFFIC_OUT_Z_SCORE: &str = "trafficOutZScore";

// --- Saúde do equipamento ---------------------------------------------------
//
// São campos **de dispositivo**, não do servidor. Quem os publica é qualquer
// checagem que meça a saúde de um equipamento: a coleta local do NetMonitor e
// o dataset do SNMP de um roteador, hoje; o que vier depois, amanhã. Foi por
// isso que o produto não tinha alerta de CPU até aqui — a leitura existia, mas
// fora do vocabulário avaliável, e regra alguma podia falar sobre ela.

/// Uso de CPU em % (0–100).
pub const CPU_USAGE_PERCENT: &str = "cpuUsagePercent";
/// Memória usada em % (0–100).
pub const MEMORY_USED_PERCENT: &str = "memoryUsedPercent";
/// Armazenamento usado em % (0–100).
pub const STORAGE_USED_PERCENT: &str = "storageUsedPercent";
/// Carga média de 1 minuto, em processos executáveis.
pub const LOAD_AVERAGE_1M: &str = "loadAverage1m";

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

// --- Padrões no log recebido por syslog -------------------------------------

/// Chave do padrão que casou, igual à do template (`log_login_failure`).
pub const LOG_PATTERN_KEY: &str = "logPatternKey";
/// Quantas vezes o padrão casou dentro da janela deslizante. É o campo que a
/// regra compara ("N ocorrências em M minutos").
pub const LOG_MATCH_COUNT: &str = "logMatchCount";
/// Largura da janela observada, em segundos.
pub const LOG_WINDOW_SECONDS: &str = "logWindowSeconds";
/// A severidade mais grave entre as linhas que casaram (0 = emergência).
pub const LOG_SEVERITY: &str = "logSeverity";
/// A última mensagem que casou — vai para o texto do alerta.
pub const LOG_MESSAGE: &str = "logMessage";
pub const VPN_PREVIOUS_STATUS: &str = "vpnPreviousStatus";

/// Vocabulário completo oferecido na tela de regras. A **ordem importa**: é a
/// ordem em que os campos aparecem no seletor da interface.
pub const ALERT_FIELDS: [&str; 52] = [
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
    CPU_USAGE_PERCENT,
    MEMORY_USED_PERCENT,
    STORAGE_USED_PERCENT,
    LOAD_AVERAGE_1M,
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
    LOG_PATTERN_KEY,
    LOG_MATCH_COUNT,
    LOG_WINDOW_SECONDS,
    LOG_SEVERITY,
    LOG_MESSAGE,
    LATENCY_BASELINE_MS,
    LATENCY_STDDEV_MS,
    LATENCY_DEVIATION_PERCENT,
    LATENCY_Z_SCORE,
    LATENCY_UPPER_BAND_MS,
    PACKET_LOSS_BASELINE_PERCENT,
    PACKET_LOSS_STDDEV_PERCENT,
    PACKET_LOSS_DEVIATION_PERCENT,
    PACKET_LOSS_Z_SCORE,
    PACKET_LOSS_UPPER_BAND_PERCENT,
    UPTIME_BASELINE_PERCENT,
    UPTIME_STDDEV_PERCENT,
    UPTIME_DEVIATION_PERCENT,
    UPTIME_Z_SCORE,
    SYSLOG_VOLUME_BASELINE,
    SYSLOG_VOLUME_STDDEV,
    SYSLOG_VOLUME_Z_SCORE,
    TRAFFIC_IN_Z_SCORE,
    TRAFFIC_OUT_Z_SCORE,
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
