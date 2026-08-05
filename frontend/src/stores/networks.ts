import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { useCrudResource } from './crudResource'

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
  const resource = useCrudResource<Network>('/networks', {
    fetch: 'Erro ao carregar redes',
    create: 'Erro ao criar rede',
    update: 'Erro ao atualizar rede',
    delete: 'Erro ao excluir rede',
  })
  const scanningId = ref<number | null>(null)

  async function fetchNetworks() {
    await resource.fetchAll()
  }

  async function createNetwork(payload: Partial<Network>): Promise<boolean> {
    return (await resource.create(payload)) !== null
  }

  async function updateNetwork(id: number, payload: Partial<Network>): Promise<boolean> {
    return (await resource.update(id, payload)) !== null
  }

  async function deleteNetwork(id: number): Promise<boolean> {
    return resource.remove(id)
  }

  async function scanNetwork(id: number): Promise<boolean> {
    scanningId.value = id
    try {
      await apiService.post(`/networks/${id}/scan`)
      return true
    } catch (err: unknown) {
      resource.error.value =
        err instanceof Error ? err.message : 'Erro ao iniciar varredura da rede'
      return false
    } finally {
      scanningId.value = null
    }
  }

  return {
    networks: resource.items,
    loading: resource.loading,
    scanningId,
    error: resource.error,
    fetchNetworks,
    createNetwork,
    updateNetwork,
    deleteNetwork,
    scanNetwork,
  }
})
