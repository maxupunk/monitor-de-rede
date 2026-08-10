/**
 * Vocabulário avaliável pelas regras de alerta.
 *
 * Cada chave aqui é um `condition.field` válido. Os produtores de fatos
 * (dataset builders) só publicam chaves desta lista e o catálogo só constrói
 * condições a partir dela — assim a UI, o avaliador e os templates continuam
 * falando a mesma língua. Os rótulos em português vivem no front
 * (`frontend/src/utils/alertPresentation.ts`), que espelha estas chaves.
 */
export const ALERT_FIELDS = {
  // --- Resultado de monitor -------------------------------------------------
  /** Situação apurada na checagem: up | down | warning | unknown */
  status: 'status',
  latencyMs: 'latencyMs',
  packetLoss: 'packetLoss',
  statusCode: 'statusCode',
  durationMs: 'durationMs',
  connectTimeMs: 'connectTimeMs',
  resolutionTimeMs: 'resolutionTimeMs',
  /** Leitura SNMP pontual do monitor: 1 = up, 2 = down */
  ifOperStatus: 'ifOperStatus',
  ifSpeed: 'ifSpeed',
  snmpUptime: 'snmpUptime',
  inBps: 'inBps',
  outBps: 'outBps',

  // --- Estado das interfaces coletadas via SNMP -----------------------------
  /** Nome da interface — permite restringir a regra a uplinks, por exemplo */
  interfaceName: 'interfaceName',
  interfaceOperStatus: 'interfaceOperStatus',
  /** Transição observada no ciclo: up_to_down | down_to_up */
  interfaceStatusTransition: 'interfaceStatusTransition',
  /** Velocidade negociada no ciclo atual, em bps */
  interfaceSpeedBps: 'interfaceSpeedBps',
  /** Renegociação observada no ciclo: downgrade | upgrade */
  interfaceSpeedTransition: 'interfaceSpeedTransition',
  /** Quanto a velocidade caiu, em % da anterior (apenas em downgrade) */
  interfaceSpeedDropPercent: 'interfaceSpeedDropPercent',

  // --- Túneis WireGuard -----------------------------------------------------
  /** Nome do equipamento do outro lado do túnel */
  vpnPeerName: 'vpnPeerName',
  /** Estado atual do túnel: connected | unstable | disconnected | awaiting */
  vpnPeerStatus: 'vpnPeerStatus',
  /** Transição observada no ciclo: connected_to_disconnected, unstable_to_connected, ... */
  vpnStatusTransition: 'vpnStatusTransition',
  /** Segundos desde o último sinal de vida (keepalive ou handshake) */
  vpnSecondsSinceActivity: 'vpnSecondsSinceActivity',
} as const

export type AlertField = (typeof ALERT_FIELDS)[keyof typeof ALERT_FIELDS]

export const INTERFACE_STATUS_TRANSITION = {
  wentDown: 'up_to_down',
  cameBack: 'down_to_up',
} as const

export const INTERFACE_SPEED_TRANSITION = {
  downgrade: 'downgrade',
  upgrade: 'upgrade',
} as const

/**
 * Transições de túnel que valem como fato alertável.
 *
 * `awaiting` (peer criado e nunca conectado) não entra: não houve queda, o
 * túnel simplesmente ainda não subiu — alertar aí seria alarme de instalação.
 */
export const VPN_STATUS_TRANSITION = {
  disconnected: 'connected_to_disconnected',
  destabilized: 'connected_to_unstable',
  reconnected: 'reconnected',
} as const
