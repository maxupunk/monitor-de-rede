import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { getStoredToken } from '@/utils/authStorage'

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

export interface DiscoveryIdentity {
  operatingSystem?: string
  label?: string
  source?: string
  reason?: string
  sysDescr?: string
  sysObjectId?: string
  sysName?: string
  hardwareVendor?: string
  hardwareModel?: string
}

/** Lê a identidade tanto do snapshot SSE quanto do resultado persistido. */
export function discoveryIdentity(
  result: Pick<DiscoveryResult | StreamedDiscoveryHost, 'data'> | null | undefined
): DiscoveryIdentity | null {
  const data = result?.data
  if (!data || typeof data !== 'object') return null
  const details = data.details
  const container = details && typeof details === 'object' ? details : data
  const identity = (container as Record<string, unknown>).identity
  return identity && typeof identity === 'object' ? (identity as DiscoveryIdentity) : null
}

export interface DeviceTypePresentation {
  label: string
  icon: string
  color: string
  isKnown: boolean
}

/**
 * Retorna o nome mais específico do dispositivo descoberto:
 * 1. Nome SNMP (sysName)
 * 2. Hostname registrado (DNS)
 * 3. Nome mDNS/Bonjour
 * 4. null caso nenhum esteja disponível.
 */
export function discoveryDeviceName(
  result:
    | Pick<DiscoveryResult | StreamedDiscoveryHost, 'data'>
    | { data?: Record<string, unknown> | null; hostname?: string | null; mdnsName?: string | null }
    | null
    | undefined
): string | null {
  if (!result) return null
  const identity = discoveryIdentity(result)
  const sysName = identity?.sysName?.trim()
  if (sysName) return sysName
  const host = result as { hostname?: string | null; mdnsName?: string | null }
  const hostname = host.hostname?.trim()
  if (hostname) return hostname
  const mdns = host.mdnsName?.trim()
  if (mdns) return mdns
  return null
}

/**
 * Mapeamento canônico de ícone, cor e rótulo para apresentação de deviceType.
 */
export function discoveryDeviceTypeInfo(deviceType?: string | null): DeviceTypePresentation {
  const type = deviceType?.toLowerCase()?.trim()
  switch (type) {
    case 'router':
      return { label: 'Roteador', icon: 'mdi-router-network', color: 'primary', isKnown: true }
    case 'switch':
    case 'unmanaged_switch':
      return { label: 'Switch', icon: 'mdi-hub', color: 'teal', isKnown: true }
    case 'access_point':
    case 'ap':
      return { label: 'Access Point', icon: 'mdi-access-point', color: 'cyan', isKnown: true }
    case 'camera':
      return { label: 'Câmera', icon: 'mdi-cctv', color: 'purple', isKnown: true }
    case 'server':
      return { label: 'Servidor', icon: 'mdi-server', color: 'deep-purple', isKnown: true }
    case 'printer':
      return { label: 'Impressora', icon: 'mdi-printer', color: 'orange', isKnown: true }
    case 'web_device':
      return { label: 'Dispositivo Web', icon: 'mdi-web', color: 'blue', isKnown: true }
    case 'firewall':
      return { label: 'Firewall', icon: 'mdi-shield-network', color: 'red', isKnown: true }
    case 'other':
      return { label: 'Outro', icon: 'mdi-devices', color: 'blue-grey', isKnown: true }
    default:
      return { label: 'Desconhecido', icon: 'mdi-lan', color: 'grey', isKnown: false }
  }
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
    const token = getStoredToken()
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
    conflicts,
    environment,
    loadingConflicts,
    fetchDiscoveryRuns,
    cleanup,
    fetchScanState,
    startScan,
    subscribeScanStream,
    cancelScan,
    fetchEnvironment,
    fetchConflicts,
    checkConflicts,
  }
})
