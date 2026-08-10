import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { useCrudResource } from './crudResource'

export interface Network {
  id: number
  siteId: number | null
  probeId?: number | null
  name: string
  cidr: string
  gateway?: string
  vlanId?: number
  description?: string
  scanEnabled?: boolean
  scanInterval?: number
  lastScanAt?: string | null
  nextScanAt?: string | null
  /** Derivado no backend: a faixa CIDR é utilizável numa varredura */
  scannable?: boolean
  /** Derivado no backend: endereços da faixa, antes do teto de varredura */
  usableHosts?: number
  site?: { id: number; name: string }
  createdAt?: string
  updatedAt?: string
}

/** Resposta de `POST /api/networks/:id/scan` */
export interface NetworkScanResult {
  message: string
  alreadyQueued: boolean
  run: { id: number; status: string }
  usableHosts: number
  truncated: boolean
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

  /**
   * Enfileira a varredura da faixa. O servidor responde de imediato (202): quem
   * varre é o scheduler, então a resposta descreve o agendamento, não o
   * resultado — os equipamentos encontrados aparecem em `/discovery`.
   */
  async function scanNetwork(id: number): Promise<NetworkScanResult | null> {
    scanningId.value = id
    try {
      return await apiService.post<NetworkScanResult>(`/networks/${id}/scan`)
    } catch (err: unknown) {
      resource.error.value =
        err instanceof Error ? err.message : 'Erro ao iniciar varredura da rede'
      return null
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
