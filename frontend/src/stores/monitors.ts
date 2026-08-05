import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface MonitorResult {
  id: number
  monitorId: number
  probeId?: number | null
  status: 'up' | 'down' | 'warning' | 'unknown' | 'disabled'
  startedAt: string
  finishedAt: string
  durationMs: number
  latencyMs: number | null
  message?: string | null
  data?: Record<string, unknown>
}

export interface MonitorStats {
  avgLatency: number | null
  minLatency: number | null
  maxLatency: number | null
  lastLatency: number | null
  uptimePercentage: number
  totalChecks: number
  upChecks: number
}

export interface Monitor {
  id: number
  deviceId: number
  probeId?: number
  name: string
  type: 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'
  target: string
  port?: number
  configuration?: Record<string, unknown>
  intervalSeconds: number
  timeoutSeconds: number
  retryCount?: number
  status: 'online' | 'offline' | 'warning' | 'disabled' | 'up' | 'down' | 'unknown'
  enabled?: boolean
  isEnabled: boolean
  lastCheckedAt?: string
  lastLatencyMs?: number
  device?: { id: number; name: string }
  probe?: { id: number; name: string }
  recentResults?: MonitorResult[]
  stats?: MonitorStats
  gaugeMetric?: { name: string; value: number; unit: string; recordedAt: string } | null
  createdAt?: string
  updatedAt?: string
}

/** Teto do histórico mantido em memória — acompanha o limite do backend */
const HISTORY_LIMIT = 100

export const useMonitorsStore = defineStore('monitors', () => {
  const monitors = ref<Monitor[]>([])
  const currentMonitor = ref<Monitor | null>(null)
  const loading = ref(false)
  const runningId = ref<number | null>(null)
  const error = ref<string | null>(null)

  function formatMonitor(m: any): Monitor {
    const isEnabled = m.isEnabled ?? m.enabled ?? true
    return {
      ...m,
      enabled: isEnabled,
      isEnabled,
    }
  }

  async function fetchMonitors() {
    loading.value = true
    error.value = null
    try {
      const data = await apiService.get<any[]>('/monitors')
      monitors.value = data.map(formatMonitor)
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar monitores'
    } finally {
      loading.value = false
    }
  }

  async function fetchMonitorById(id: number): Promise<Monitor | null> {
    loading.value = true
    error.value = null
    try {
      const data = await apiService.get<any>(`/monitors/${id}`)
      const formatted = formatMonitor(data)
      currentMonitor.value = formatted
      return formatted
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar detalhes do monitor'
      return null
    } finally {
      loading.value = false
    }
  }

  async function createMonitor(payload: Partial<Monitor>): Promise<boolean> {
    try {
      const created = await apiService.post<any>('/monitors', payload)
      monitors.value.push(formatMonitor(created))
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar monitor'
      return false
    }
  }

  async function updateMonitor(id: number, payload: Partial<Monitor>): Promise<boolean> {
    try {
      const updated = await apiService.put<any>(`/monitors/${id}`, payload)
      const formatted = formatMonitor(updated)
      const index = monitors.value.findIndex((m) => m.id === id)
      if (index !== -1) {
        monitors.value[index] = formatted
      }
      if (currentMonitor.value?.id === id) {
        currentMonitor.value = { ...currentMonitor.value, ...formatted }
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar monitor'
      return false
    }
  }

  async function deleteMonitor(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/monitors/${id}`)
      monitors.value = monitors.value.filter((m) => m.id !== id)
      if (currentMonitor.value?.id === id) {
        currentMonitor.value = null
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir monitor'
      return false
    }
  }

  async function runMonitor(id: number): Promise<boolean> {
    runningId.value = id
    try {
      await apiService.post(`/monitors/${id}/run`)
      if (currentMonitor.value?.id === id) {
        await fetchMonitorById(id)
      } else {
        await fetchMonitors()
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao executar verificação do monitor'
      return false
    } finally {
      runningId.value = null
    }
  }

  async function toggleMonitorEnabled(id: number, enable: boolean): Promise<boolean> {
    try {
      const endpoint = enable ? `/monitors/${id}/enable` : `/monitors/${id}/disable`
      const updated = await apiService.post<any>(endpoint)
      const formatted = formatMonitor(updated)
      const mon = monitors.value.find((m) => m.id === id)
      if (mon) {
        mon.enabled = formatted.isEnabled
        mon.isEnabled = formatted.isEnabled
        mon.status = formatted.status
      }
      if (currentMonitor.value?.id === id) {
        currentMonitor.value.enabled = formatted.isEnabled
        currentMonitor.value.isEnabled = formatted.isEnabled
        currentMonitor.value.status = formatted.status
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao alterar estado do monitor'
      return false
    }
  }

  /** Mesmo cálculo do MonitorsController.show, para os KPIs não ficarem defasados */
  function computeStats(results: MonitorResult[]): MonitorStats {
    const latencies = results
      .map((r) => r.latencyMs)
      .filter((l): l is number => l !== null && l !== undefined)

    const totalChecks = results.length
    const upChecks = results.filter((r) => r.status === 'up').length

    return {
      avgLatency:
        latencies.length > 0
          ? Math.round(latencies.reduce((a, b) => a + b, 0) / latencies.length)
          : null,
      minLatency: latencies.length > 0 ? Math.min(...latencies) : null,
      maxLatency: latencies.length > 0 ? Math.max(...latencies) : null,
      // `results` está do mais antigo para o mais recente
      lastLatency: latencies.length > 0 ? latencies[latencies.length - 1] : null,
      uptimePercentage: totalChecks > 0 ? Number(((upChecks / totalChecks) * 100).toFixed(1)) : 100,
      totalChecks,
      upChecks,
    }
  }

  /**
   * Aplica o payload SSE `monitor:result` diretamente no monitor em memória:
   * status, última latência, barra de histórico, gráfico e KPIs. Evita que a
   * tela dependa de recarregamento manual para refletir o monitoramento.
   */
  function applyRealtimeResult(data: Record<string, unknown>) {
    const id = Number(data.monitorId ?? data.id)
    if (!id) return

    const status = data.status as Monitor['status']
    const latencyMs = data.latencyMs === null ? undefined : (data.latencyMs as number | undefined)
    const finishedAt = String(data.finishedAt ?? new Date().toISOString())

    const result: MonitorResult = {
      // Id sintético: o registro real só chega no próximo carregamento da lista
      id: Date.now(),
      monitorId: id,
      status: status as MonitorResult['status'],
      startedAt: String(data.startedAt ?? finishedAt),
      finishedAt,
      durationMs: Number(data.durationMs ?? 0),
      latencyMs: latencyMs ?? null,
      message: (data.message as string | null) ?? null,
    }

    const patch = (target: Monitor, withStats: boolean) => {
      if (status) target.status = status
      target.lastLatencyMs = latencyMs
      target.lastCheckedAt = finishedAt

      // A timeline exibe do mais antigo para o mais recente
      const history = [...(target.recentResults || []), result].slice(-HISTORY_LIMIT)
      target.recentResults = history

      if (withStats) target.stats = computeStats(history)
    }

    const listed = monitors.value.find((m) => m.id === id)
    if (listed) patch(listed, false)
    if (currentMonitor.value?.id === id) patch(currentMonitor.value, true)
  }

  /** Atualiza a última leitura de CPU/Memória exibida nos monitores de uso */
  function applyRealtimeMetrics(data: Record<string, unknown>) {
    const deviceId = Number(data.deviceId)
    const metrics = (data.metrics as Array<Record<string, unknown>>) || []
    if (!deviceId || metrics.length === 0) return

    const apply = (target: Monitor) => {
      if (target.deviceId !== deviceId) return
      const name = target.gaugeMetric?.name
      if (!name) return

      const sample = [...metrics].reverse().find((m) => m.name === name)
      if (!sample) return

      target.gaugeMetric = {
        name,
        value: Number(sample.value),
        unit: String(sample.unit ?? '%'),
        recordedAt: String(sample.recordedAt),
      }
    }

    for (const mon of monitors.value) apply(mon)
    if (currentMonitor.value) apply(currentMonitor.value)
  }

  return {
    monitors,
    currentMonitor,
    loading,
    runningId,
    error,
    applyRealtimeResult,
    applyRealtimeMetrics,
    fetchMonitors,
    fetchMonitorById,
    createMonitor,
    updateMonitor,
    deleteMonitor,
    runMonitor,
    toggleMonitorEnabled,
  }
})
