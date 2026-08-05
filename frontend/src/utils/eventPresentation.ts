import type { RealtimeEventPayload } from '@/stores/events'
import { formatClockTime, formatMeasuredValue } from '@/utils/formatters'

export interface FormattedEventDetails {
  title: string
  message: string
  icon: string
  color: string
  time: string
  metrics?: Array<{
    name: string
    value: number | string
    unit?: string
    interfaceId?: number | null
  }>
  rawJson: string
}

export function formatEventDetails(evt: RealtimeEventPayload): FormattedEventDetails {
  const d = (evt.data || {}) as Record<string, any>
  const dateStr = formatClockTime(evt.timestamp)

  const rawJson = JSON.stringify(d, null, 2)

  switch (evt.type) {
    case 'metric:recorded': {
      const deviceName =
        d.deviceName || (d.deviceId ? `Dispositivo #${d.deviceId}` : 'Métricas Coletadas')
      const rawMetrics = Array.isArray(d.metrics) ? d.metrics : []
      const metrics = rawMetrics.map((m: any) => ({
        name: String(m.name || m.metricName || 'métrica'),
        value: m.value !== undefined ? m.value : m.metricValue,
        unit: m.unit || '',
        interfaceId: m.interfaceId ?? null,
      }))

      let message = 'Nenhuma métrica fornecida'
      if (metrics.length > 0) {
        const interfaceIds = [
          ...new Set(metrics.map((m: any) => m.interfaceId).filter((id: any) => id != null)),
        ]
        const cpu = metrics.find((m: any) => m.name === 'cpu_usage')
        const mem = metrics.find((m: any) => m.name === 'memory_usage')

        const highlights: string[] = []
        if (cpu) highlights.push(`CPU: ${cpu.value}%`)
        if (mem) highlights.push(`Memória: ${mem.value}%`)

        if (highlights.length > 0) {
          const ifText = interfaceIds.length > 0 ? ` • ${interfaceIds.length} interface(s)` : ''
          message = `${highlights.join(' • ')} (${metrics.length} métricas${ifText})`
        } else if (interfaceIds.length > 0) {
          const firstIf = metrics.find((m: any) => m.interfaceId != null)
          const firstVal = firstIf
            ? `${firstIf.name} #${firstIf.interfaceId}: ${formatMeasuredValue(firstIf.value, firstIf.unit)}`
            : ''
          message = `${metrics.length} métrica(s) em ${interfaceIds.length} interface(s) (${firstVal})`
        } else {
          const formattedList = metrics
            .slice(0, 3)
            .map((m: any) => `${m.name}: ${formatMeasuredValue(m.value, m.unit)}`)
            .join(', ')
          const moreCount = metrics.length > 3 ? ` (+${metrics.length - 3} mais)` : ''
          message = `${metrics.length} métrica${metrics.length > 1 ? 's' : ''}: ${formattedList}${moreCount}`
        }
      }

      return {
        title: deviceName,
        message,
        icon: 'mdi-chart-line',
        color: 'info',
        time: dateStr,
        metrics,
        rawJson,
      }
    }
    case 'monitor:result': {
      const name = d.name || d.monitorName || `Monitor #${d.monitorId || d.id || ''}`
      const status = String(d.status || '').toLowerCase()
      const isUp = status === 'up' || status === 'online'
      const latency = d.latencyMs !== undefined && d.latencyMs !== null ? `${d.latencyMs} ms` : null
      return {
        title: name,
        message: `${isUp ? 'Verificação OK (ONLINE)' : 'Falha na Verificação (OFFLINE)'}${latency ? ` • Latência: ${latency}` : ''}`,
        icon: isUp ? 'mdi-check-circle' : 'mdi-alert-circle',
        color: isUp ? 'success' : 'error',
        time: dateStr,
        rawJson,
      }
    }
    case 'device:status': {
      const name = d.name || d.deviceName || `Dispositivo #${d.id || ''}`
      const status = String(d.status || '').toLowerCase()
      const isOnline = status === 'online'
      return {
        title: name,
        message: `Status do equipamento alterado para ${status.toUpperCase()}`,
        icon: isOnline ? 'mdi-lan-connect' : 'mdi-lan-disconnect',
        color: isOnline ? 'success' : 'error',
        time: dateStr,
        rawJson,
      }
    }
    case 'alert:triggered': {
      return {
        title: d.title || d.ruleName || 'Novo Alerta Disparado',
        message: d.message || 'Aviso de incidente registrado no sistema',
        icon: 'mdi-bell-ring',
        color: 'warning',
        time: dateStr,
        rawJson,
      }
    }
    case 'alert:resolved': {
      return {
        title: d.title || 'Alerta Normalizado',
        message: d.message || 'A condição que gerou o alerta foi restabelecida',
        icon: 'mdi-check-decagram',
        color: 'success',
        time: dateStr,
        rawJson,
      }
    }
    case 'alert:acknowledged': {
      return {
        title: d.title || 'Alerta Reconhecido',
        message: d.message || 'Alerta foi reconhecido por um operador',
        icon: 'mdi-check-circle-outline',
        color: 'primary',
        time: dateStr,
        rawJson,
      }
    }
    case 'alert:silenced': {
      return {
        title: d.title || 'Alerta Silenciado',
        message: d.message || 'Notificações silenciadas temporariamente',
        icon: 'mdi-bell-off-outline',
        color: 'warning',
        time: dateStr,
        rawJson,
      }
    }
    case 'interface:status_change':
    case 'interface:speed_change':
    case 'interface:speed_downgrade': {
      return {
        title: `Interface ${d.ifName || d.interfaceName || ''}`.trim(),
        message: String(d.message || 'Alteração detectada na interface'),
        icon: 'mdi-ethernet-cable',
        color: evt.type === 'interface:speed_change' ? 'info' : 'warning',
        time: dateStr,
        rawJson,
      }
    }
    case 'probe:status': {
      return {
        title: String(d.name || `Probe #${d.id || ''}`),
        message: `Probe passou para o estado ${String(d.status || '').toUpperCase()}`,
        icon: 'mdi-router-wireless',
        color: d.status === 'online' ? 'success' : 'warning',
        time: dateStr,
        rawJson,
      }
    }
    case 'discovery:started':
    case 'discovery:completed':
    case 'discovery:failed': {
      const isOk = evt.type === 'discovery:completed'
      const isFail = evt.type === 'discovery:failed'
      return {
        title: 'Varredura de Rede',
        message:
          d.message ||
          (isOk
            ? 'Descoberta finalizada com sucesso'
            : isFail
              ? 'Erro durante a descoberta'
              : 'Descoberta iniciada'),
        icon: isOk ? 'mdi-radar' : isFail ? 'mdi-radar' : 'mdi-radar',
        color: isOk ? 'success' : isFail ? 'error' : 'info',
        time: dateStr,
        rawJson,
      }
    }
    case 'vpn:peers_updated': {
      return {
        title: 'VPN WireGuard',
        message: d.message || 'Atualização no estado dos peers de VPN',
        icon: 'mdi-shield-key-outline',
        color: 'primary',
        time: dateStr,
        rawJson,
      }
    }
    case 'topology:updated': {
      return {
        title: 'Topologia de Rede',
        message: d.message || 'Grafo de conexões entre dispositivos atualizado',
        icon: 'mdi-graphql',
        color: 'info',
        time: dateStr,
        rawJson,
      }
    }
    default: {
      const summary =
        d.name ||
        d.title ||
        d.message ||
        (Object.keys(d).length
          ? `Dados do evento (${Object.keys(d).length} atributos)`
          : 'Evento de sistema')
      return {
        title: evt.type || 'Evento do Sistema',
        message: String(summary),
        icon: 'mdi-pulse',
        color: 'info',
        time: dateStr,
        rawJson,
      }
    }
  }
}
