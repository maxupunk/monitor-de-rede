import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { useDevicesStore } from './devices'
import { useAlertsStore, type AlertEvent } from './alerts'
import { useMonitorsStore } from './monitors'
import { useProbesStore } from './probes'
import { useDiscoveryStore } from './discovery'
import { useDeviceDetailStore } from './deviceDetail'
import { useTopologyStore } from './topology'
import { useVpnStore } from './vpn'
import { useMaintenanceWindowsStore } from './maintenanceWindows'
import { useDockerStore } from './docker'

export interface RealtimeEventPayload {
  /** Nome do evento publicado pelo backend (ex.: `monitor:result`) */
  type: string
  data: Record<string, unknown>
  timestamp: string
}

export interface HourlyDistributionBin {
  label: string
  hour: number
  timestamp: string
  critical: number
  warning: number
  info: number
}

export interface HourlyDistributionTotals {
  critical: number
  warning: number
  info: number
  total: number
}

export interface HourlyDistributionResponse {
  hours: number
  bins: HourlyDistributionBin[]
  totals: HourlyDistributionTotals
}

/** Máximo de eventos mantidos no feed em memória */
const FEED_LIMIT = 50
/** Janela de agrupamento para recargas que não podem ser aplicadas em memória */
const REFRESH_DEBOUNCE_MS = 800
const MAX_RECONNECT_DELAY_MS = 30_000

export const useEventsStore = defineStore('events', () => {
  const isConnected = ref(false)
  const recentEvents = ref<RealtimeEventPayload[]>([])
  const lastEventAt = ref<string | null>(null)

  let eventSource: EventSource | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let reconnectAttempts = 0
  const pendingRefreshes = new Map<string, ReturnType<typeof setTimeout>>()

  type EventHandler = (data: Record<string, unknown>, payload: RealtimeEventPayload) => void
  const handlers = new Map<string, Set<EventHandler>>()

  /**
   * Assinatura pontual usada por telas que mantêm estado local (gráficos, por
   * exemplo). Retorna a função de cancelamento para usar no `onUnmounted`.
   */
  function onEvent(types: string | string[], handler: EventHandler): () => void {
    const list = Array.isArray(types) ? types : [types]
    for (const type of list) {
      if (!handlers.has(type)) handlers.set(type, new Set())
      handlers.get(type)!.add(handler)
    }

    return () => {
      for (const type of list) handlers.get(type)?.delete(handler)
    }
  }

  function notifyHandlers(payload: RealtimeEventPayload) {
    for (const handler of handlers.get(payload.type) ?? []) {
      try {
        handler(payload.data, payload)
      } catch {
        // Um assinante com defeito não pode interromper os demais
      }
    }
  }

  /**
   * Agrupa recargas disparadas em rajada (uma varredura pode emitir dezenas de
   * eventos por segundo) em uma única chamada por recurso.
   */
  function scheduleRefresh(key: string, action: () => void) {
    const existing = pendingRefreshes.get(key)
    if (existing) clearTimeout(existing)
    pendingRefreshes.set(
      key,
      setTimeout(() => {
        pendingRefreshes.delete(key)
        action()
      }, REFRESH_DEBOUNCE_MS)
    )
  }

  function connect() {
    if (eventSource) return

    const token = localStorage.getItem('auth_token')
    const url = token
      ? `/api/events/stream?token=${encodeURIComponent(token)}`
      : '/api/events/stream'

    eventSource = new EventSource(url)

    eventSource.onopen = () => {
      isConnected.value = true
      reconnectAttempts = 0
    }

    eventSource.onmessage = (msg) => {
      try {
        const raw = JSON.parse(msg.data) as Record<string, unknown>
        const payload: RealtimeEventPayload = {
          // `event` é aceito por compatibilidade com payloads antigos
          type: String(raw.type ?? raw.event ?? 'unknown'),
          data: (raw.data as Record<string, unknown>) ?? {},
          timestamp: String(raw.timestamp ?? new Date().toISOString()),
        }
        handleIncomingEvent(payload)
      } catch {
        // Mensagem não parseável — ignora
      }
    }

    eventSource.onerror = () => {
      isConnected.value = false
      closeSource()
      scheduleReconnect()
    }
  }

  function scheduleReconnect() {
    if (reconnectTimer) return
    // Backoff exponencial: 1s, 2s, 4s ... até 30s
    const delay = Math.min(1000 * 2 ** reconnectAttempts, MAX_RECONNECT_DELAY_MS)
    reconnectAttempts += 1
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null
      connect()
    }, delay)
  }

  function handleIncomingEvent(payload: RealtimeEventPayload) {
    lastEventAt.value = payload.timestamp

    const isEphemeralTelemetry =
      payload.type === 'docker:snapshot' || payload.type === 'docker:inventory'
    if (payload.type !== 'stream:connected' && !isEphemeralTelemetry) {
      recentEvents.value.unshift(payload)
      if (recentEvents.value.length > FEED_LIMIT) {
        recentEvents.value.length = FEED_LIMIT
      }
    }

    const data = payload.data || {}

    switch (payload.type) {
      case 'monitor:result': {
        const monitorsStore = useMonitorsStore()
        const deviceDetailStore = useDeviceDetailStore()
        monitorsStore.applyRealtimeResult(data)
        deviceDetailStore.applyMonitorResult(data)
        break
      }

      case 'device:status':
      case 'device:updated': {
        const devicesStore = useDevicesStore()
        const deviceDetailStore = useDeviceDetailStore()
        const topologyStore = useTopologyStore()
        devicesStore.applyRealtimeStatus(data)
        deviceDetailStore.applyDeviceStatus(data)
        topologyStore.applyRealtimeStatus(data)
        break
      }

      case 'alert:triggered': {
        const alertsStore = useAlertsStore()
        // Traz o registro completo (com relações) sem esperar refetch geral
        scheduleRefresh('alerts', () => alertsStore.fetchActiveAlerts())
        alertsStore.upsertAlertEvent({
          id: Number(data.id ?? data.alertEventId),
          alertRuleId: (data.alertRuleId as number) ?? null,
          deviceId: (data.deviceId as number) ?? null,
          monitorId: (data.monitorId as number) ?? null,
          severity: (data.severity as 'info' | 'warning' | 'critical') ?? 'warning',
          status: (data.status as AlertEvent['status']) ?? 'active',
          title: String(data.title ?? data.ruleName ?? 'Novo alerta'),
          message: String(data.message ?? ''),
          data: (data.data as AlertEvent['data']) ?? null,
          startedAt: data.startedAt as string,
          createdAt: String(data.createdAt ?? data.startedAt ?? payload.timestamp),
        })
        break
      }

      case 'alert:updated': {
        const alertsStore = useAlertsStore()
        // Payload é o evento serializado completo: patch em memória, sem refetch
        alertsStore.upsertAlertEvent(data as never)
        break
      }

      case 'alert:resolved': {
        const alertsStore = useAlertsStore()
        alertsStore.patchAlertEvent(Number(data.id ?? data.alertEventId), {
          status: 'resolved',
          resolvedAt: data.resolvedAt as string,
        })
        break
      }

      case 'alert:acknowledged':
      case 'alert:silenced': {
        const alertsStore = useAlertsStore()
        alertsStore.patchAlertEvent(Number(data.id ?? data.alertEventId), {
          status: payload.type === 'alert:silenced' ? 'silenced' : 'acknowledged',
          silencedUntil: (data.silencedUntil as string) ?? undefined,
        })
        break
      }

      case 'alert_rule:created':
      case 'alert_rule:updated': {
        const alertsStore = useAlertsStore()
        alertsStore.upsertAlertRule(data as never)
        break
      }

      case 'alert_rule:deleted': {
        const alertsStore = useAlertsStore()
        alertsStore.removeAlertRule(Number(data.id))
        break
      }

      case 'probe:status': {
        const probesStore = useProbesStore()
        probesStore.applyRealtimeStatus(data)
        break
      }

      case 'discovery:started':
      case 'discovery:completed':
      case 'discovery:failed': {
        const discoveryStore = useDiscoveryStore()
        scheduleRefresh('discovery', () => {
          discoveryStore.fetchDiscoveryRuns()
        })
        break
      }

      case 'interface:status_change':
      case 'interface:speed_change':
      case 'interface:speed_downgrade': {
        const deviceDetailStore = useDeviceDetailStore()
        deviceDetailStore.applyInterfaceChange(payload.type, data)
        break
      }

      case 'metric:recorded': {
        const deviceDetailStore = useDeviceDetailStore()
        const monitorsStore = useMonitorsStore()
        deviceDetailStore.applyRecordedMetrics(data)
        monitorsStore.applyRealtimeMetrics(data)
        break
      }

      case 'vpn:peers_updated': {
        const vpnStore = useVpnStore()
        vpnStore.applyRealtimePeers(data)
        break
      }

      case 'topology:updated': {
        const topologyStore = useTopologyStore()
        // Novos enlaces exigem recarregar o grafo: não dá para deduzir
        // a geometria do mapa a partir do evento.
        scheduleRefresh('topology', () => topologyStore.fetchTopology())
        break
      }

      case 'maintenance_windows:updated': {
        const maintenanceStore = useMaintenanceWindowsStore()
        scheduleRefresh('maintenance_windows', () => maintenanceStore.fetchWindows())
        break
      }

      case 'docker:snapshot': {
        const dockerStore = useDockerStore()
        dockerStore.applyLiveSnapshot(data as never)
        break
      }

      case 'docker:inventory': {
        const dockerStore = useDockerStore()
        dockerStore.applyInventorySnapshot(data as never)
        break
      }
    }

    notifyHandlers(payload)
  }

  function closeSource() {
    if (eventSource) {
      eventSource.close()
      eventSource = null
    }
  }

  function disconnect() {
    closeSource()
    if (reconnectTimer) {
      clearTimeout(reconnectTimer)
      reconnectTimer = null
    }
    for (const timer of pendingRefreshes.values()) clearTimeout(timer)
    pendingRefreshes.clear()
    reconnectAttempts = 0
    isConnected.value = false
  }

  async function fetchHourlyDistribution(hours = 6): Promise<HourlyDistributionResponse | null> {
    try {
      return await apiService.get<HourlyDistributionResponse>(
        `/events/hourly-distribution?hours=${hours}`
      )
    } catch {
      return null
    }
  }

  return {
    isConnected,
    recentEvents,
    lastEventAt,
    connect,
    disconnect,
    onEvent,
    fetchHourlyDistribution,
  }
})
