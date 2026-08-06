import type { Monitor, MonitorResult } from '@/stores/monitors'

/**
 * Monitores SNMP de uso de CPU/Memória são leituras de percentual (gauge), não
 * checagens up/down — este helper identifica esses monitores para que a UI
 * mostre a leitura atual em vez de um status de disponibilidade.
 */
export function isGaugeMonitor(
  monitor: Pick<Monitor, 'type' | 'configuration' | 'gaugeMetric'>
): boolean {
  const metric = (monitor.configuration?.metric as string | undefined) || monitor.gaugeMetric?.name
  return (
    monitor.type === 'snmp' &&
    (metric === 'cpu_usage' || metric === 'memory_usage' || metric === 'interface_traffic')
  )
}

export function gaugeMetricName(monitor: Pick<Monitor, 'configuration' | 'gaugeMetric'>): string {
  return (
    (monitor.configuration?.metric as string | undefined) ||
    monitor.gaugeMetric?.name ||
    'cpu_usage'
  )
}

export function gaugeTypeLabel(monitor: Pick<Monitor, 'configuration' | 'gaugeMetric'>): string {
  const name = gaugeMetricName(monitor)
  if (name === 'memory_usage') return 'MEMÓRIA'
  if (name === 'interface_traffic') return 'TRÁFEGO'
  return 'CPU'
}

/** Limiares de alerta de uso replicados do card de CPU/Memória do DeviceDetailPage. */
export function gaugeColor(value: number | null | undefined, metricName: string): string {
  if (value === null || value === undefined) return 'grey'
  if (metricName === 'memory_usage') {
    if (value > 90) return 'error'
    if (value > 75) return 'warning'
    return 'success'
  }
  if (value > 85) return 'error'
  if (value > 65) return 'warning'
  return 'success'
}

export function isInterfaceMonitor(monitor: Pick<Monitor, 'type' | 'configuration'>): boolean {
  const ifIndex = monitor.configuration?.ifIndex
  return monitor.type === 'snmp' && ifIndex !== undefined && ifIndex !== null
}

export interface InterfaceStatusInfo {
  label: string
  color: string
  icon: string
}

/**
 * Uma interface de rede não é só up/down: pode estar desabilitada pelo admin, em
 * teste, dormente ou sem hardware presente (ver mapeamento de ifOperStatus em
 * snmp_checker.ts) — e quando está up, a velocidade negociada é o dado relevante.
 */
export function interfaceStatusInfo(
  status: Monitor['status'] | undefined,
  data: Record<string, unknown> | undefined
): InterfaceStatusInfo {
  const operText = (data?.operStatusText as string | undefined) || null
  const speed = (data?.speedFormatted as string | undefined) || null

  switch (status) {
    case 'up':
      return { label: speed || 'Up', color: 'success', icon: 'mdi-lan-connect' }
    case 'disabled':
      return { label: 'Desabilitada', color: 'grey', icon: 'mdi-power-plug-off-outline' }
    case 'down':
      return { label: operText || 'Down', color: 'error', icon: 'mdi-lan-disconnect' }
    case 'warning':
      return { label: operText || 'Instável', color: 'warning', icon: 'mdi-lan-pending' }
    default:
      return { label: operText || 'Desconhecido', color: 'grey', icon: 'mdi-help-network-outline' }
  }
}

export function latestResultData(
  results: MonitorResult[] | undefined
): Record<string, unknown> | undefined {
  if (!results || results.length === 0) return undefined
  return results[results.length - 1]?.data
}

/**
 * Categorias de estado reconhecidas na UI. Todo status textual vindo do backend
 * (dispositivo, monitor, alerta, interface, porta, probe) é reduzido a uma
 * destas categorias antes de virar cor — assim a mesma situação nunca aparece
 * verde numa tela e cinza em outra.
 */
export type StatusTone = 'success' | 'error' | 'warning' | 'info' | 'neutral'

const STATUS_TONES: Record<string, StatusTone> = {
  up: 'success',
  online: 'success',
  success: 'success',
  ok: 'success',
  active: 'success',
  accepted: 'success',
  open: 'success',
  connected: 'success',
  down: 'error',
  offline: 'error',
  error: 'error',
  critical: 'error',
  failed: 'error',
  disconnected: 'error',
  warning: 'warning',
  degraded: 'warning',
  unstable: 'warning',
  instável: 'warning',
  silenced: 'warning',
  filtered: 'warning',
  merged: 'info',
  disabled: 'neutral',
  desabilitada: 'neutral',
  inactive: 'neutral',
  revoked: 'neutral',
  unknown: 'neutral',
  pending: 'neutral',
  closed: 'neutral',
  awaiting: 'neutral',
}

/** Cores do tema Vuetify por categoria */
const TONE_THEME_COLORS: Record<StatusTone, string> = {
  success: 'success',
  error: 'error',
  warning: 'warning',
  info: 'info',
  neutral: 'grey',
}

/**
 * Cores literais para contextos que não resolvem o tema do Vuetify — SVG,
 * canvas e estilos inline.
 */
const TONE_HEX_COLORS: Record<StatusTone, string> = {
  success: '#4CAF50',
  error: '#F44336',
  warning: '#FF9800',
  info: '#2196F3',
  neutral: '#B0BEC5',
}

/** Classifica qualquer status textual numa das categorias visuais */
export function statusTone(status?: string | null): StatusTone {
  if (!status) return 'neutral'
  return STATUS_TONES[status.toLowerCase().trim()] ?? 'neutral'
}

/**
 * Retorna a cor do Vuetify correspondente ao estado/status de um dispositivo,
 * monitor, alerta ou interface de forma unificada.
 */
export function getStatusColor(status?: string | null): string {
  return TONE_THEME_COLORS[statusTone(status)]
}

/** Mesma classificação de `getStatusColor`, em hexadecimal para SVG/canvas */
export function getStatusHexColor(status?: string | null): string {
  return TONE_HEX_COLORS[statusTone(status)]
}
