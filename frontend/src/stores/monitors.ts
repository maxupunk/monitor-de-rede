import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface Monitor {
  id: number
  deviceId: number
  probeId?: number
  name: string
  type: 'ping' | 'http' | 'tcp' | 'dns' | 'snmp'
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
  createdAt?: string
  updatedAt?: string
}

export const useMonitorsStore = defineStore('monitors', () => {
  const monitors = ref<Monitor[]>([])
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
      const index = monitors.value.findIndex((m) => m.id === id)
      if (index !== -1) {
        monitors.value[index] = formatMonitor(updated)
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
      await fetchMonitors()
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
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao alterar estado do monitor'
      return false
    }
  }

  return {
    monitors,
    loading,
    runningId,
    error,
    fetchMonitors,
    createMonitor,
    updateMonitor,
    deleteMonitor,
    runMonitor,
    toggleMonitorEnabled,
  }
})
