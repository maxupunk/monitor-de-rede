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
      await apiService.post(endpoint)
      const mon = monitors.value.find((m) => m.id === id)
      if (mon) {
        mon.enabled = enable
        mon.isEnabled = enable
        if (!enable) mon.status = 'disabled'
      }
      if (currentMonitor.value?.id === id) {
        currentMonitor.value.enabled = enable
        currentMonitor.value.isEnabled = enable
        if (!enable) currentMonitor.value.status = 'disabled'
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao alterar estado do monitor'
      return false
    }
  }

  return {
    monitors,
    currentMonitor,
    loading,
    runningId,
    error,
    fetchMonitors,
    fetchMonitorById,
    createMonitor,
    updateMonitor,
    deleteMonitor,
    runMonitor,
    toggleMonitorEnabled,
  }
})
