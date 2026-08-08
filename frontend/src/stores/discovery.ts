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
 * Os nomes espelham as colunas de `discovery_results`. A versão anterior
 * declarava `suggestedName` / `confidenceScore`, que a API nunca enviou — as
 * colunas correspondentes na tela apareciam sempre vazias.
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
  status: 'pending' | 'accepted' | 'ignored' | 'merged'
  discoveryRun?: DiscoveryRun | null
  firstSeenAt?: string
  lastSeenAt?: string
  createdAt?: string
}

export const useDiscoveryStore = defineStore('discovery', () => {
  const runs = ref<DiscoveryRun[]>([])
  const results = ref<DiscoveryResult[]>([])
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

  async function fetchDiscoveryResults() {
    loading.value = true
    error.value = null
    try {
      results.value = await apiService.get<DiscoveryResult[]>('/discovery/results')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar resultados de descoberta'
    } finally {
      loading.value = false
    }
  }

  async function acceptResult(
    resultId: number,
    payload?: { siteId?: number; name?: string }
  ): Promise<boolean> {
    try {
      await apiService.post(`/discovery/results/${resultId}/accept`, payload)
      const res = results.value.find((r) => r.id === resultId)
      if (res) res.status = 'accepted'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao aceitar resultado de descoberta'
      return false
    }
  }

  async function ignoreResult(resultId: number): Promise<boolean> {
    try {
      await apiService.post(`/discovery/results/${resultId}/ignore`)
      const res = results.value.find((r) => r.id === resultId)
      if (res) res.status = 'ignored'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao ignorar resultado'
      return false
    }
  }

  async function markAccepted(resultId: number): Promise<boolean> {
    try {
      await apiService.post(`/discovery/results/${resultId}/mark-accepted`)
      const res = results.value.find((r) => r.id === resultId)
      if (res) res.status = 'accepted'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao marcar resultado como adicionado'
      return false
    }
  }

  async function mergeResult(resultId: number, targetDeviceId: number): Promise<boolean> {
    try {
      await apiService.post(`/discovery/results/${resultId}/merge`, { targetDeviceId })
      const res = results.value.find((r) => r.id === resultId)
      if (res) res.status = 'merged'
      return true
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao mesclar resultado com dispositivo existente'
      return false
    }
  }

  return {
    runs,
    results,
    loading,
    error,
    fetchDiscoveryRuns,
    fetchDiscoveryResults,
    acceptResult,
    ignoreResult,
    markAccepted,
    mergeResult,
  }
})
