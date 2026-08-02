import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface Monitor {
  id: number
  deviceId: number
  probeId?: number
  name: string
  type: 'ping' | 'http' | 'tcp' | 'dns'
  target: string
  port?: number
  intervalSeconds: number
  timeoutSeconds: number
  status: 'online' | 'offline' | 'warning' | 'disabled'
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

  async function fetchMonitors() {
    loading.value = true
    error.value = null
    try {
      monitors.value = await apiService.get<Monitor[]>('/monitors')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar monitores'
    } finally {
      loading.value = false
    }
  }

  async function createMonitor(payload: Partial<Monitor>): Promise<boolean> {
    try {
      const created = await apiService.post<Monitor>('/monitors', payload)
      monitors.value.push(created)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar monitor'
      return false
    }
  }

  async function updateMonitor(id: number, payload: Partial<Monitor>): Promise<boolean> {
    try {
      const updated = await apiService.put<Monitor>(`/monitors/${id}`, payload)
      const index = monitors.value.findIndex((m) => m.id === id)
      if (index !== -1) {
        monitors.value[index] = updated
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
