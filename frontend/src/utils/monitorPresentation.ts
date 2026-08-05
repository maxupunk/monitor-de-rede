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
  return monitor.type === 'snmp' && (metric === 'cpu_usage' || metric === 'memory_usage')
}

export function gaugeMetricName(monitor: Pick<Monitor, 'configuration' | 'gaugeMetric'>): string {
  return (
    (monitor.configuration?.metric as string | undefined) ||
    monitor.gaugeMetric?.name ||
    'cpu_usage'
  )
}

export function gaugeTypeLabel(monitor: Pick<Monitor, 'configuration' | 'gaugeMetric'>): string {
  return gaugeMetricName(monitor) === 'memory_usage' ? 'MEMÓRIA' : 'CPU'
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
