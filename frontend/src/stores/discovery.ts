import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import { compareIpAddresses, type DiscoveredHostLike } from '@/utils/discoveryPresentation'

/** `sweep` acha quem está vivo; `identify` descobre o que cada vivo é. */
export type DiscoveryPhase = 'sweep' | 'identify' | 'probe' | 'idle'

export type DiscoveryRunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'

export interface DiscoveryRun {
  id: number
  networkId: number
  networkName?: string | null
  probeId?: number | null
  status: DiscoveryRunStatus
  /** Derivado no backend a partir da contagem de `discovery_results` */
  devicesFound: number
  /** Faixa varrida, vinda da configuração da run */
  cidr?: string | null
  startedAt?: string
  finishedAt?: string | null
  error?: string | null
}

/** Um host da varredura ao vivo (o snapshot `discovery:scan`). */
export interface StreamedDiscoveryHost extends DiscoveredHostLike {
  ipAddress: string
  macAddress?: string | null
  hostname?: string | null
  mdnsName?: string | null
  vendor?: string | null
  deviceType?: string | null
  openPorts?: number[]
  confidence: number
  data?: Record<string, unknown>
}

export interface NetworkConflict {
  id: string
  conflictType: 'ipCollision' | 'macDuplicated' | 'deviceMacMismatch'
  ipAddress: string
  macAddresses: string[]
  affectedDeviceId?: number | null
  affectedDeviceName?: string | null
  vendors: string[]
  severity: string
  description: string
  detectedAt: string
}

export interface DiscoveryEnvironment {
  isHostMode: boolean
  containerized: boolean
}

export type ScanStatus = 'idle' | 'pending' | 'running' | 'completed' | 'cancelled' | 'failed'

export interface ScanSessionState {
  runId: number | null
  networkId: number | null
  status: ScanStatus
  phase: DiscoveryPhase
  progressCurrent: number
  progressTotal: number
  hosts: StreamedDiscoveryHost[]
  logs: string[]
  error: string | null
  startedAt: string | null
  finishedAt: string | null
}

const IDLE_SCAN: ScanSessionState = {
  runId: null,
  networkId: null,
  status: 'idle',
  phase: 'idle',
  progressCurrent: 0,
  progressTotal: 0,
  hosts: [],
  logs: [],
  error: null,
  startedAt: null,
  finishedAt: null,
}

function isScanState(value: unknown): value is ScanSessionState {
  if (typeof value !== 'object' || value === null) return false
  const state = value as Partial<ScanSessionState>
  return typeof state.status === 'string' && Array.isArray(state.hosts)
}

export const useDiscoveryStore = defineStore('discovery', () => {
  const runs = ref<DiscoveryRun[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  /**
   * A varredura ao vivo. Chega só pelo stream global (`discovery:scan`): o
   * backend publica o estado inteiro a cada quadro e o repete a quem conecta.
   */
  const scan = ref<ScanSessionState>({ ...IDLE_SCAN })
  /** Pedido de início em voo — a tela reage antes do primeiro quadro chegar. */
  const starting = ref(false)

  const scanning = computed(
    () => starting.value || scan.value.status === 'running' || scan.value.status === 'pending'
  )
  const hosts = computed(() =>
    [...scan.value.hosts].sort((a, b) => compareIpAddresses(a.ipAddress, b.ipAddress))
  )
  const progressPercent = computed(() => {
    const { progressCurrent, progressTotal, status } = scan.value
    if (status === 'completed') return 100
    return progressTotal > 0 ? Math.min(100, (progressCurrent / progressTotal) * 100) : 0
  })

  /** Aplica um quadro do stream; dados malformados são ignorados. */
  function applyScanSnapshot(data: unknown) {
    if (!isScanState(data)) return
    scan.value = { ...IDLE_SCAN, ...data }
    if (data.status !== 'idle') starting.value = false
  }

  async function fetchDiscoveryRuns(): Promise<DiscoveryRun[]> {
    loading.value = true
    error.value = null
    try {
      runs.value = await apiService.get<DiscoveryRun[]>('/discovery/runs')
      return runs.value
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar execuções de descoberta'
      return []
    } finally {
      loading.value = false
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

  /**
   * Pede a varredura. Ela roda no servidor mesmo que a aba feche; o progresso
   * chega pelo stream global.
   */
  async function startScan(networkId: number): Promise<number | null> {
    error.value = null
    starting.value = true
    try {
      const response = await apiService.post<{ runId: number; status: string }>('/discovery/scan', {
        networkId,
      })
      return response.runId
    } catch (err: unknown) {
      starting.value = false
      error.value = err instanceof Error ? err.message : 'Erro ao iniciar varredura'
      return null
    }
  }

  async function cancelScan(): Promise<boolean> {
    try {
      await apiService.post('/discovery/scan-cancel')
      starting.value = false
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao cancelar varredura'
      return false
    }
  }

  const conflicts = ref<NetworkConflict[]>([])
  const environment = ref<DiscoveryEnvironment | null>(null)
  const loadingConflicts = ref(false)

  async function fetchEnvironment(): Promise<DiscoveryEnvironment | null> {
    try {
      environment.value = await apiService.get<DiscoveryEnvironment>('/discovery/environment')
      return environment.value
    } catch (err: unknown) {
      console.error('Erro ao consultar ambiente de rede:', err)
      return null
    }
  }

  async function fetchConflicts(): Promise<NetworkConflict[]> {
    loadingConflicts.value = true
    try {
      conflicts.value = await apiService.get<NetworkConflict[]>('/discovery/conflicts')
      return conflicts.value
    } catch (err: unknown) {
      console.error('Erro ao carregar conflitos de rede:', err)
      return []
    } finally {
      loadingConflicts.value = false
    }
  }

  async function checkConflicts(): Promise<NetworkConflict[]> {
    loadingConflicts.value = true
    try {
      const res = await apiService.post<{ count: number; conflicts: NetworkConflict[] }>(
        '/discovery/check-conflicts'
      )
      conflicts.value = res.conflicts
      return conflicts.value
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao escanear conflitos de rede'
      return []
    } finally {
      loadingConflicts.value = false
    }
  }

  return {
    runs,
    loading,
    error,
    scan,
    scanning,
    hosts,
    progressPercent,
    conflicts,
    environment,
    loadingConflicts,
    applyScanSnapshot,
    fetchDiscoveryRuns,
    cleanup,
    startScan,
    cancelScan,
    fetchEnvironment,
    fetchConflicts,
    checkConflicts,
  }
})
