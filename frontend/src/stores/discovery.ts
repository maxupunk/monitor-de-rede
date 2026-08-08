import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface DiscoveryRun {
  id: number
  networkId: number
  probeId?: number | null
  status: 'pending' | 'running' | 'completed' | 'failed'
  /** Derivado no backend a partir da contagem de `discovery_results` */
  devicesFound: number
  /** Faixa varrida, vinda da configuração da run ou da rede */
  cidr?: string | null
  networkName?: string | null
  startedAt?: string
  finishedAt?: string | null
  error?: string | null
  network?: {
    id: number
    name: string
    cidr: string
    siteId?: number
    site?: { id: number; name: string }
  } | null
}

/**
 * Cache do último scan de descoberta. Não há status persistente: um resultado
 * existe enquanto o IP ainda não foi transformado em device.
 */
export interface DiscoveryResult {
  id: number
  discoveryRunId: number
  ipAddress: string
  macAddress?: string | null
  hostname?: string | null
  mdnsName?: string | null
  vendor?: string | null
  deviceType?: string | null
  confidence: number
  discoveryRun?: DiscoveryRun | null
  firstSeenAt?: string
  lastSeenAt?: string
  createdAt?: string
  data?: Record<string, unknown> | null
}

export const useDiscoveryStore = defineStore('discovery', () => {
  const runs = ref<DiscoveryRun[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchDiscoveryRuns() {
    loading.value = true
    error.value = null
    try {
      runs.value = await apiService.get<DiscoveryRun[]>('/discovery/runs')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar execuções de descoberta'
    } finally {
      loading.value = false
    }
  }

  async function acceptResult(resultId: number): Promise<boolean> {
    try {
      await apiService.post(`/discovery/results/${resultId}/accept`)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao aceitar resultado de descoberta'
      return false
    }
  }

  async function mergeResult(resultId: number, targetDeviceId: number): Promise<boolean> {
    try {
      await apiService.post(`/discovery/results/${resultId}/merge`, { targetDeviceId })
      return true
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao mesclar resultado com dispositivo existente'
      return false
    }
  }

  async function cleanup(olderThanDays = 7): Promise<{ removedRuns: number } | null> {
    try {
      return await apiService.delete<{ removedRuns: number }>(
        `/discovery/cleanup?olderThanDays=${olderThanDays}`
      )
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao limpar histórico de descoberta'
      return null
    }
  }

  return {
    runs,
    loading,
    error,
    fetchDiscoveryRuns,
    acceptResult,
    mergeResult,
    cleanup,
  }
})
