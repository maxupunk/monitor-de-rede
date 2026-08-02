import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface Site {
  id: number
  name: string
  location?: string
  description?: string
  devicesCount?: number
  networksCount?: number
  createdAt?: string
  updatedAt?: string
}

export const useSitesStore = defineStore('sites', () => {
  const sites = ref<Site[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchSites() {
    loading.value = true
    error.value = null
    try {
      sites.value = await apiService.get<Site[]>('/sites')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar sites'
    } finally {
      loading.value = false
    }
  }

  async function createSite(payload: Partial<Site>): Promise<boolean> {
    try {
      const created = await apiService.post<Site>('/sites', payload)
      sites.value.push(created)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar site'
      return false
    }
  }

  async function updateSite(id: number, payload: Partial<Site>): Promise<boolean> {
    try {
      const updated = await apiService.put<Site>(`/sites/${id}`, payload)
      const index = sites.value.findIndex((s) => s.id === id)
      if (index !== -1) {
        sites.value[index] = updated
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar site'
      return false
    }
  }

  async function deleteSite(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/sites/${id}`)
      sites.value = sites.value.filter((s) => s.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir site'
      return false
    }
  }

  return { sites, loading, error, fetchSites, createSite, updateSite, deleteSite }
})
