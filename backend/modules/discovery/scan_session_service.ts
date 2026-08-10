import { DateTime } from 'luxon'
import type { DiscoveryCallbacks } from './discovery_service.js'
import type { DiscoveredHost } from './scanners/icmp_scanner.js'

export type DiscoveryPhase = 'icmp' | 'discovery' | 'ports' | 'snmp' | 'idle'

export type ScanSessionStatus = 'idle' | 'running' | 'completed' | 'cancelled' | 'failed'

export interface ScannedHost {
  ipAddress: string
  macAddress?: string
  hostname?: string
  mdnsName?: string
  vendor?: string
  deviceType?: string
  openPorts?: number[]
  confidence: number
  data?: Record<string, unknown>
}

export interface ScanSessionState {
  runId: number | null
  networkId: number | null
  status: ScanSessionStatus
  phase: DiscoveryPhase
  progressCurrent: number
  progressTotal: number
  hosts: ScannedHost[]
  logs: string[]
  error: string | null
  startedAt: string | null
  finishedAt: string | null
}

const INITIAL_STATE: ScanSessionState = {
  runId: null,
  networkId: null,
  status: 'idle',
  phase: 'idle',
  progressCurrent: 0,
  progressTotal: 100,
  hosts: [],
  logs: [],
  error: null,
  startedAt: null,
  finishedAt: null,
}

/**
 * Mantém o estado da varredura ativa em memória no backend.
 *
 * O scan roda desacoplado da conexão do cliente: o frontend pode sair da página
 * e, ao voltar, recuperar o progresso e os hosts encontrados via
 * `GET /api/discovery/scan-state` ou reconectar no SSE.
 *
 * Apenas uma sessão existe por vez. Iniciar uma nova varredura limpa a sessão
 * anterior.
 */
export class ScanSessionService implements DiscoveryCallbacks {
  private state: ScanSessionState = { ...INITIAL_STATE }
  private abortController: AbortController | null = null
  private listeners = new Set<() => void>()

  /**
   * Inicia uma nova sessão, descartando qualquer sessão anterior.
   */
  startSession(runId: number, networkId: number): AbortSignal | undefined {
    this.clearSession()
    this.abortController = new AbortController()
    this.state = {
      ...INITIAL_STATE,
      runId,
      networkId,
      status: 'running',
      phase: 'icmp',
      startedAt: DateTime.now().toISO(),
    }
    this.addLog('Iniciando varredura...')
    this.notify()
    return this.abortController.signal
  }

  /**
   * Limpa a sessão atual. Não aborta o scan em andamento — use `cancel()`
   * quando quiser também interromper o trabalho.
   */
  clearSession(): void {
    this.abortController = null
    this.state = { ...INITIAL_STATE }
    this.notify()
  }

  /**
   * Cancela o scan em andamento sinalizando o AbortSignal.
   */
  cancel(): void {
    if (this.state.status !== 'running') return
    this.abortController?.abort()
    this.state.status = 'cancelled'
    this.state.finishedAt = DateTime.now().toISO()
    this.addLog('Varredura cancelada.')
    this.notify()
  }

  get signal(): AbortSignal | undefined {
    return this.abortController?.signal
  }

  getState(): ScanSessionState {
    return {
      ...this.state,
      hosts: [...this.state.hosts].sort((a, b) => a.ipAddress.localeCompare(b.ipAddress)),
      logs: [...this.state.logs],
    }
  }

  /**
   * Retorna os callbacks que o DiscoveryService deve invocar durante a
   * varredura. Assim o serviço atualiza o cache e notifica ouvintes em tempo
   * real sem expor estado mutável.
   */
  asCallbacks(): DiscoveryCallbacks {
    return {
      onProgress: (phase, current, total) => this.onProgress(phase as DiscoveryPhase, current, total),
      onResult: (host) => this.onResult(host),
    }
  }

  onProgress(phase: string, current: number, total: number): void {
    if (this.state.status !== 'running') return
    this.state.phase = this.toPhase(phase)
    this.state.progressCurrent = current
    this.state.progressTotal = total
    this.notify()
  }

  private toPhase(phase: string): DiscoveryPhase {
    const valid: DiscoveryPhase[] = ['icmp', 'discovery', 'ports', 'snmp']
    return valid.includes(phase as DiscoveryPhase) ? (phase as DiscoveryPhase) : 'idle'
  }

  onResult(host: DiscoveredHost): void {
    if (this.state.status !== 'running') return
    const existingIndex = this.state.hosts.findIndex((h) => h.ipAddress === host.ipAddress)
    const mapped: ScannedHost = {
      ipAddress: host.ipAddress,
      macAddress: host.macAddress,
      hostname: host.hostname,
      mdnsName: host.mdnsName,
      vendor: host.vendor,
      deviceType: host.deviceType,
      openPorts: host.openPorts,
      confidence: host.confidence,
      data: host.data,
    }
    if (existingIndex >= 0) {
      this.state.hosts[existingIndex] = mapped
    } else {
      this.state.hosts.push(mapped)
      this.addLog(`Encontrado: ${host.ipAddress}`)
    }
    this.notify()
  }

  complete(): void {
    if (this.state.status !== 'running') return
    this.state.status = 'completed'
    this.state.finishedAt = DateTime.now().toISO()
    this.addLog('Varredura finalizada.')
    this.notify()
  }

  fail(error: string): void {
    if (this.state.status !== 'running') return
    this.state.status = 'failed'
    this.state.error = error
    this.state.finishedAt = DateTime.now().toISO()
    this.addLog(`Erro: ${error}`)
    this.notify()
  }

  private addLog(message: string): void {
    this.state.logs = [...this.state.logs.slice(-19), message]
  }

  /**
   * Permite que o controller SSE se inscreva para receber eventos sempre que o
   * estado mudar.
   */
  subscribe(callback: () => void): () => void {
    this.listeners.add(callback)
    return () => {
      this.listeners.delete(callback)
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener()
      } catch {
        // Ignora falhas de ouvintes para não corromper o estado.
      }
    }
  }
}

/**
 * Instância singleton compartilhada pelo controller e pelo worker de scan.
 */
export const scanSessionService = new ScanSessionService()
