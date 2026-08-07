import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { Device } from './devices'
import type { Monitor } from './monitors'

export interface DeviceInterface {
  id: number
  deviceId: number
  snmpIndex?: number
  ifIndex?: number
  name?: string
  ifName?: string
  ifType?: string
  adminStatus?: string
  ifAdminStatus?: 'up' | 'down' | 'testing'
  operStatus?: string
  ifOperStatus?: 'up' | 'down' | 'testing'
  speed?: number
  ifSpeed?: number
  macAddress?: string
  ipAddress?: string
  inOctets?: number
  outOctets?: number
  inBps?: number
  outBps?: number
}

export interface DeviceMetric {
  id: number
  deviceId: number
  interfaceId?: number | null
  interfaceName?: string | null
  metricName: string
  metricValue: number
  unit?: string
  createdAt: string
}

/**
 * `GET /api/devices/:id/monitors` devolve o mesmo payload de `GET /api/monitors`
 * (ver `modules/monitoring/monitor_presenter.ts`), porque a aba "Monitores" do
 * equipamento usa o mesmo componente de listagem de `/monitors`.
 */
export type DeviceMonitor = Monitor

export interface DeviceEvent {
  id: number
  deviceId: number
  eventType: string
  severity: 'info' | 'warning' | 'error' | 'critical'
  message: string
  createdAt: string
}

export interface ScanInterfaceItem {
  ifIndex: number
  ifName: string
  ifDescr?: string
  macAddress?: string
  ifSpeed?: number
  ifAdminStatus?: string
  ifOperStatus?: string
  isMonitored: boolean
}

export interface ScanZabbixTemplateItem {
  id: number
  name: string
  key: string
  units: string | null
  value: number | null
}

export interface ScanResult {
  systemInfo: {
    sysName?: string
    sysDescr?: string
    sysUpTime?: number
  }
  cpuInfo: {
    usagePercent?: number
    userPercent?: number
    systemPercent?: number
    idlePercent?: number
    load1min?: number
    coresCount?: number
  }
  memoryInfo: {
    totalKb?: number
    usedKb?: number
    usedPercent?: number
  }
  interfaces: ScanInterfaceItem[]
  hasCpuMonitor: boolean
  hasMemoryMonitor: boolean
  zabbixTemplateItems: ScanZabbixTemplateItem[]
  snmpResponded: boolean
}

/** Teto de amostras mantidas em memória na tela de detalhe */
const METRICS_LIMIT = 2000

/** Teto do histórico por monitor mantido em memória — acompanha o do backend */
const MONITOR_HISTORY_LIMIT = 100

export const useDeviceDetailStore = defineStore('deviceDetail', () => {
  const device = ref<Device | null>(null)
  const interfaces = ref<DeviceInterface[]>([])
  const metrics = ref<DeviceMetric[]>([])
  const monitors = ref<DeviceMonitor[]>([])
  const events = ref<DeviceEvent[]>([])
  const loading = ref(false)
  const pollingSnmp = ref(false)
  const scanningSnmp = ref(false)
  const scanResult = ref<ScanResult | null>(null)
  const error = ref<string | null>(null)

  /**
   * O backend serializa `enabled`; a UI (e o `MonitorsTable`) lê `isEnabled`.
   * Mesma normalização feita no store de monitores — sem ela o switch de
   * ativação abriria desligado em monitores ativos.
   */
  function normalizeMonitors(payload: unknown): DeviceMonitor[] {
    if (!Array.isArray(payload)) return []
    return payload.map((mon: any) => {
      const isEnabled = mon.isEnabled ?? mon.enabled ?? true
      return { ...mon, enabled: isEnabled, isEnabled }
    })
  }

  /** Recarrega apenas os monitores, após uma ação disparada pela listagem. */
  async function reloadMonitors(deviceId: number): Promise<void> {
    try {
      const data = await apiService.get<DeviceMonitor[]>(`/devices/${deviceId}/monitors`)
      monitors.value = normalizeMonitors(data)
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao recarregar os monitores'
    }
  }

  async function loadDeviceDetails(deviceId: number) {
    loading.value = true
    error.value = null
    try {
      const [devData, intfData, monData, metData, evtData] = await Promise.allSettled([
        apiService.get<Device>(`/devices/${deviceId}`),
        apiService.get<DeviceInterface[]>(`/devices/${deviceId}/interfaces`),
        apiService.get<DeviceMonitor[]>(`/devices/${deviceId}/monitors`),
        apiService.get<DeviceMetric[]>(`/devices/${deviceId}/metrics`),
        apiService.get<DeviceEvent[]>(`/devices/${deviceId}/events`),
      ])

      if (devData.status === 'fulfilled') device.value = devData.value
      if (intfData.status === 'fulfilled') {
        interfaces.value = Array.isArray(intfData.value) ? intfData.value : []
      }
      if (monData.status === 'fulfilled') {
        monitors.value = normalizeMonitors(monData.value)
      }
      if (metData.status === 'fulfilled') {
        metrics.value = Array.isArray(metData.value) ? metData.value : []
      }
      if (evtData.status === 'fulfilled') {
        events.value = Array.isArray(evtData.value) ? evtData.value : []
      }
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar detalhes do dispositivo'
    } finally {
      loading.value = false
    }
  }

  async function triggerSnmpPoll(deviceId: number): Promise<boolean> {
    pollingSnmp.value = true
    try {
      await apiService.post(`/devices/${deviceId}/snmp/poll`)
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao executar poll SNMP'
      return false
    } finally {
      pollingSnmp.value = false
    }
  }

  async function scanDeviceSnmp(deviceId: number): Promise<ScanResult | null> {
    scanningSnmp.value = true
    error.value = null
    try {
      const res = await apiService.post<ScanResult>(`/devices/${deviceId}/snmp/scan`)
      scanResult.value = res
      return res
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao escanear dispositivo via SNMP'
      return null
    } finally {
      scanningSnmp.value = false
    }
  }

  async function applySnmpMonitors(
    deviceId: number,
    options: {
      enableCpuMonitor?: boolean
      enableMemoryMonitor?: boolean
      monitoredIfIndexes?: number[]
    }
  ): Promise<boolean> {
    loading.value = true
    try {
      await apiService.post(`/devices/${deviceId}/snmp/apply-monitors`, options)
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao aplicar configurações de monitoramento'
      return false
    } finally {
      loading.value = false
    }
  }

  // --- Aplicação dos eventos SSE na tela de detalhe aberta ---

  /** Só reage a eventos do dispositivo atualmente carregado */
  function isCurrentDevice(data: Record<string, unknown>): boolean {
    const id = Number(data.deviceId ?? data.id)
    return !!device.value && !!id && device.value.id === id
  }

  function applyDeviceStatus(data: Record<string, unknown>) {
    if (!isCurrentDevice(data) || !device.value) return
    if (data.status) device.value.status = data.status as Device['status']
  }

  function applyMonitorResult(data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return
    const monitorId = Number(data.monitorId ?? data.id)
    const monitor = monitors.value.find((m) => m.id === monitorId)
    if (!monitor) return

    const latencyMs = (data.latencyMs as number | null | undefined) ?? null
    const finishedAt = String(data.finishedAt ?? new Date().toISOString())

    if (data.status) monitor.status = data.status as DeviceMonitor['status']
    monitor.latencyMs = latencyMs
    monitor.lastLatencyMs = latencyMs ?? undefined
    monitor.lastCheckedAt = finishedAt

    // A listagem desta aba usa o mesmo componente de `/monitors`, que desenha a
    // linha do tempo a partir de `recentResults` — sem anexar a amostra aqui, a
    // barra ficaria congelada enquanto a tela estivesse aberta.
    monitor.recentResults = [
      ...(monitor.recentResults ?? []),
      {
        // Id sintético: o registro real só chega no próximo carregamento
        id: Date.now(),
        monitorId,
        status: data.status as DeviceMonitor['status'] as any,
        startedAt: String(data.startedAt ?? finishedAt),
        finishedAt,
        durationMs: Number(data.durationMs ?? 0),
        latencyMs,
        message: (data.message as string | null) ?? null,
      },
    ].slice(-MONITOR_HISTORY_LIMIT)
  }

  /**
   * Anexa as amostras do evento `metric:recorded` ao histórico já carregado,
   * mantendo gráficos de CPU/Memória e tráfego vivos sem recarregar a página.
   */
  function applyRecordedMetrics(data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return

    const samples = (data.metrics as Array<Record<string, unknown>>) || []
    if (samples.length === 0) return

    const deviceId = Number(data.deviceId)
    for (const sample of samples) {
      metrics.value.push({
        id: Date.now() + metrics.value.length,
        deviceId,
        interfaceId: (sample.interfaceId as number | null) ?? null,
        interfaceName: null,
        metricName: String(sample.name),
        metricValue: Number(sample.value),
        unit: String(sample.unit ?? ''),
        createdAt: String(sample.recordedAt),
      })
    }

    // Evita crescimento indefinido em telas deixadas abertas por horas
    if (metrics.value.length > METRICS_LIMIT) {
      metrics.value = metrics.value.slice(-METRICS_LIMIT)
    }
  }

  function applyInterfaceChange(type: string, data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return

    const iface = interfaces.value.find((i) => i.id === Number(data.interfaceId))
    if (iface) {
      if (type === 'interface:status_change' && data.currentStatus) {
        const status = String(data.currentStatus) as 'up' | 'down' | 'testing'
        iface.operStatus = status
        iface.ifOperStatus = status
      }
      if (type !== 'interface:status_change' && data.currentSpeed !== undefined) {
        iface.speed = Number(data.currentSpeed)
        iface.ifSpeed = Number(data.currentSpeed)
      }
    }

    // O evento também vira um registro na aba de eventos do dispositivo
    events.value.unshift({
      id: Number(data.alertEventId ?? Date.now()),
      deviceId: Number(data.deviceId),
      eventType: type,
      severity: type === 'interface:speed_change' ? 'info' : 'warning',
      message: String(data.message ?? 'Alteração detectada na interface'),
      createdAt: new Date().toISOString(),
    })
  }

  return {
    device,
    interfaces,
    metrics,
    monitors,
    events,
    loading,
    applyDeviceStatus,
    applyMonitorResult,
    applyRecordedMetrics,
    applyInterfaceChange,
    pollingSnmp,
    scanningSnmp,
    scanResult,
    error,
    loadDeviceDetails,
    reloadMonitors,
    triggerSnmpPoll,
    scanDeviceSnmp,
    applySnmpMonitors,
  }
})
