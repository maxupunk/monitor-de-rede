import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export type DiscoveryPhase = 'icmp' | 'discovery' | 'ports' | 'snmp' | 'idle'

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
    gateway?: string | null
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
  openPorts?: number[] | null
  confidence: number
  discoveryRun?: DiscoveryRun | null
  firstSeenAt?: string
  lastSeenAt?: string
  createdAt?: string
  data?: Record<string, unknown> | null
}

export interface StreamedDiscoveryHost {
  ipAddress: string
  macAddress?: string
  hostname?: string
  mdnsName?: string
  vendor?: string
  deviceType?: string
  openPorts?: number[]
  confidence: number
  data?: Record<string, unknown>
  firstSeenAt?: string
  lastSeenAt?: string
}

export interface ScanSessionState {
  runId: number | null
  networkId: number | null
  status: 'idle' | 'running' | 'completed' | 'cancelled' | 'failed'
  phase: DiscoveryPhase
  progressCurrent: number
  progressTotal: number
  hosts: StreamedDiscoveryHost[]
  logs: string[]
  error: string | null
  startedAt: string | null
  finishedAt: string | null
}

/** Espera antes de reabrir o SSE da varredura depois de uma queda */
const RECONNECT_DELAY_MS = 3_000

export const useDiscoveryStore = defineStore('discovery', () => {
  const runs = ref<DiscoveryRun[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

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
   * Busca o estado atual da varredura no backend. Usado ao montar a página
   * para restaurar o progresso quando o usuário sai e volta.
   */
  async function fetchScanState(): Promise<ScanSessionState | null> {
    try {
      const response = await apiService.get<{ data: ScanSessionState }>('/discovery/scan-state')
      return response.data ?? null
    } catch (err: unknown) {
      console.error('Erro ao carregar estado da varredura:', err)
      return null
    }
  }

  /**
   * Inicia uma varredura de forma assíncrona. O scan continua rodando no
   * servidor mesmo se o fechar a aba.
   */
  async function startScan(networkId: number): Promise<number | null> {
    error.value = null
    try {
      const response = await apiService.post<{ runId: number; status: string }>('/discovery/scan', {
        networkId,
      })
      return response.runId
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao iniciar varredura'
      return null
    }
  }

  /**
   * Conecta no SSE /discovery/scan-stream e chama os callbacks a cada
   * atualização de estado. Retorna uma função para cancelar a inscrição.
   */
  function subscribeScanStream(
    callbacks: {
      onState?: (state: ScanSessionState) => void
      onError?: (message: string) => void
    } = {}
  ): () => void {
    // `EventSource` não manda o header Authorization; o backend aceita o JWT
    // em `?token=` justamente para os streams (ver config `auth.jwt.location`).
    const token = localStorage.getItem('auth_token')
    const url = token
      ? `/api/discovery/scan-stream?token=${encodeURIComponent(token)}`
      : '/api/discovery/scan-stream'

    let source: EventSource | null = null
    let retry: ReturnType<typeof setTimeout> | null = null
    let unsubscribed = false

    const connect = () => {
      if (unsubscribed) return
      const eventSource = new EventSource(url)
      source = eventSource

      eventSource.onmessage = (event) => {
        try {
          const state = JSON.parse(event.data) as ScanSessionState
          callbacks.onState?.(state)
        } catch (err) {
          console.error('Erro ao parsear evento SSE:', err)
        }
      }

      // Uma varredura leva minutos: se a conexão cai no meio, deixar por isso
      // mesmo congela a barra de progresso até o operador recarregar a página.
      // Cada reconexão recebe o estado atual inteiro no primeiro evento.
      eventSource.onerror = () => {
        eventSource.close()
        callbacks.onError?.('Conexão com o stream de descoberta caiu; reconectando...')
        if (!unsubscribed) {
          retry = setTimeout(connect, RECONNECT_DELAY_MS)
        }
      }
    }

    connect()

    return () => {
      unsubscribed = true
      if (retry) clearTimeout(retry)
      source?.close()
    }
  }

  async function cancelScan(): Promise<void> {
    try {
      await apiService.post('/discovery/scan-cancel')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao cancelar varredura'
    }
  }

  return {
    runs,
    loading,
    error,
    fetchDiscoveryRuns,
    cleanup,
    fetchScanState,
    startScan,
    subscribeScanStream,
    cancelScan,
  }
})
