import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface Network {
  id: number
  siteId: number
  name: string
  cidr: string
  gateway?: string
  vlanId?: number
  description?: string
  site?: { id: number; name: string }
  createdAt?: string
  updatedAt?: string
}

export const useNetworksStore = defineStore('networks', () => {
  const networks = ref<Network[]>([])
  const loading = ref(false)
  const scanningId = ref<number | null>(null)
  const error = ref<string | null>(null)

  async function fetchNetworks() {
    loading.value = true
    error.value = null
    try {
      networks.value = await apiService.get<Network[]>('/networks')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar redes'
    } finally {
      loading.value = false
    }
  }

  async function createNetwork(payload: Partial<Network>): Promise<boolean> {
    try {
      const created = await apiService.post<Network>('/networks', payload)
      networks.value.push(created)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar rede'
      return false
    }
  }

  async function updateNetwork(id: number, payload: Partial<Network>): Promise<boolean> {
    try {
      const updated = await apiService.put<Network>(`/networks/${id}`, payload)
      const index = networks.value.findIndex((n) => n.id === id)
      if (index !== -1) {
        networks.value[index] = updated
      }
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar rede'
      return false
    }
  }

  async function deleteNetwork(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/networks/${id}`)
      networks.value = networks.value.filter((n) => n.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir rede'
      return false
    }
  }

  async function scanNetwork(id: number): Promise<boolean> {
    scanningId.value = id
    try {
      await apiService.post(`/networks/${id}/scan`)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao iniciar varredura da rede'
      return false
    } finally {
      scanningId.value = null
    }
  }

  return {
    networks,
    loading,
    scanningId,
    error,
    fetchNetworks,
    createNetwork,
    updateNetwork,
    deleteNetwork,
    scanNetwork,
  }
})
