import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface MaintenanceWindow {
  id: number
  siteId?: number | null
  deviceId?: number | null
  name: string
  description?: string | null
  startsAt: string
  endsAt: string
  createdBy?: number | null
  createdAt: string
  updatedAt: string
}

export interface MaintenanceWindowPayload {
  siteId?: number | null
  deviceId?: number | null
  name: string
  description?: string | null
  startsAt: string
  endsAt: string
}

export const useMaintenanceWindowsStore = defineStore('maintenanceWindows', () => {
  const windows = ref<MaintenanceWindow[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchWindows(): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      windows.value = await apiService.get<MaintenanceWindow[]>('/maintenance-windows')
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar janelas de manutenção'
      return false
    } finally {
      loading.value = false
    }
  }

  async function createWindow(
    payload: MaintenanceWindowPayload
  ): Promise<MaintenanceWindow | null> {
    error.value = null
    try {
      const created = await apiService.post<MaintenanceWindow>('/maintenance-windows', payload)
      windows.value.push(created)
      return created
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar janela de manutenção'
      return null
    }
  }

  async function updateWindow(
    id: number,
    payload: MaintenanceWindowPayload
  ): Promise<MaintenanceWindow | null> {
    error.value = null
    try {
      const updated = await apiService.put<MaintenanceWindow>(`/maintenance-windows/${id}`, payload)
      const index = windows.value.findIndex((w) => w.id === id)
      if (index !== -1) windows.value[index] = updated
      return updated
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar janela de manutenção'
      return null
    }
  }

  async function deleteWindow(id: number): Promise<boolean> {
    error.value = null
    try {
      await apiService.delete(`/maintenance-windows/${id}`)
      windows.value = windows.value.filter((w) => w.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir janela de manutenção'
      return false
    }
  }

  return {
    windows,
    loading,
    error,
    fetchWindows,
    createWindow,
    updateWindow,
    deleteWindow,
  }
})
