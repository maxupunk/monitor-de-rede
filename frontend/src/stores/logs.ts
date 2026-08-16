import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { useInfiniteCursor } from '@/composables/useInfiniteCursor'
import { apiService } from '@/services/apiService'
import type { LogEntry } from '@/bindings/LogEntry'
import type { LogSourcesResponse } from '@/bindings/LogSourcesResponse'
import type { LogSourceEntry } from '@/bindings/LogSourceEntry'
import type { SetupGuide } from '@/bindings/SetupGuide'

export type { LogEntry, LogSourceEntry, SetupGuide }

export interface LogFilters {
  deviceId: number | null
  /**
   * Severidade numérica **máxima**. No syslog o número menor é o mais grave,
   * então `3` significa "erro e acima". `null` não filtra.
   */
  severity: number | null
  /** Janela em horas contadas para trás. `null` usa o padrão do backend (24 h). */
  hours: number | null
  search: string
}

/**
 * Opções do seletor de severidade, do mais grave para o menos.
 *
 * Os rótulos individuais vêm do backend em `severityLabel` — estes aqui
 * descrevem faixas ("erro e acima"), que é outra coisa e só existe na tela.
 */
export const SEVERITY_OPTIONS = [
  { value: 2, label: 'Crítico e acima' },
  { value: 3, label: 'Erro e acima' },
  { value: 4, label: 'Aviso e acima' },
  { value: 6, label: 'Informação e acima' },
  { value: 7, label: 'Tudo, inclusive depuração' },
] as const

/** Janelas oferecidas na tela. O backend recusa qualquer coisa além de 7 dias. */
export const WINDOW_OPTIONS = [
  { value: 1, label: 'Última hora' },
  { value: 6, label: 'Últimas 6 horas' },
  { value: 24, label: 'Últimas 24 horas' },
  { value: 24 * 7, label: 'Últimos 7 dias' },
] as const

export function defaultFilters(): LogFilters {
  return { deviceId: null, severity: null, hours: 24, search: '' }
}

export const useLogsStore = defineStore('logs', () => {
  const filters = ref<LogFilters>(defaultFilters())

  /**
   * O caminho é derivado dos filtros a cada chamada, e não guardado: é assim
   * que a mesma lista segue o filtro que o usuário acabou de mudar sem ser
   * recriada.
   */
  function endpoint(): string {
    const params = new URLSearchParams()
    if (filters.value.deviceId !== null) params.set('deviceId', String(filters.value.deviceId))
    if (filters.value.severity !== null) params.set('severity', String(filters.value.severity))
    if (filters.value.hours !== null) {
      const from = new Date(Date.now() - filters.value.hours * 3_600_000)
      params.set('from', from.toISOString())
    }
    const termo = filters.value.search.trim()
    if (termo) params.set('q', termo)
    const query = params.toString()
    return query ? `/logs?${query}` : '/logs'
  }

  const list = useInfiniteCursor<LogEntry>(endpoint, { label: 'os registros de log' })

  const isEmpty = computed(() => list.items.value.length === 0)

  /** Reinicia a lista. Chamado sempre que um filtro muda. */
  function applyFilters(next: Partial<LogFilters> = {}): void {
    filters.value = { ...filters.value, ...next }
    list.reset()
  }

  function clearFilters(): void {
    filters.value = defaultFilters()
    list.reset()
  }

  // --- live tail ------------------------------------------------------------

  const tailing = ref(false)
  let tailSource: EventSource | null = null

  /**
   * Liga o live tail.
   *
   * Stream **próprio**, não o `/api/events/stream` do painel: o barramento de
   * log é separado justamente para que um tail atrasado não derrube eventos de
   * domínio (ver `services/syslog/bus.rs` no backend).
   */
  function startTail(): void {
    if (tailSource) return
    const params = new URLSearchParams()
    if (filters.value.deviceId !== null) params.set('deviceId', String(filters.value.deviceId))
    if (filters.value.severity !== null) params.set('severity', String(filters.value.severity))
    const token = localStorage.getItem('auth_token')
    if (token) params.set('token', token)

    tailSource = new EventSource(`/api/logs/stream?${params.toString()}`)
    tailSource.onmessage = (msg) => {
      try {
        const bruto = JSON.parse(msg.data) as Record<string, unknown>
        // O primeiro quadro é só o handshake com o `retry`.
        if (bruto.type === 'stream:connected') return
        list.prepend([bruto as unknown as LogEntry], (entry) => entry.id)
      } catch {
        // Quadro ilegível não pode derrubar o tail.
      }
    }
    tailSource.onerror = () => {
      // O `EventSource` reconecta sozinho; a bandeira só reflete o estado.
      tailing.value = tailSource?.readyState !== EventSource.CLOSED
    }
    tailing.value = true
  }

  function stopTail(): void {
    tailSource?.close()
    tailSource = null
    tailing.value = false
  }

  function toggleTail(): void {
    if (tailing.value) stopTail()
    else startTail()
  }

  // --- fontes vistas --------------------------------------------------------

  const sources = ref<LogSourceEntry[]>([])
  const unknownCount = ref(0)
  const sourcesLoaded = ref(false)

  async function fetchSources(): Promise<boolean> {
    try {
      const response = await apiService.get<LogSourcesResponse>('/logs/sources')
      sources.value = response.data ?? []
      unknownCount.value = response.unknownCount ?? 0
      sourcesLoaded.value = true
      return true
    } catch {
      // A ingestão pode estar desligada; a tela de logs continua útil.
      sourcesLoaded.value = true
      return false
    }
  }

  async function bindSource(sourceIp: string, deviceId: number | null): Promise<boolean> {
    try {
      await apiService.post(`/logs/sources/${encodeURIComponent(sourceIp)}/bind`, { deviceId })
      await fetchSources()
      list.reset()
      return true
    } catch {
      return false
    }
  }

  // --- guia de configuração -------------------------------------------------

  const setupGuide = ref<SetupGuide | null>(null)

  async function fetchSetupGuide(): Promise<void> {
    try {
      // O host da barra de endereços é o melhor palpite do endereço que o
      // roteador alcança: de dentro do container o processo só enxerga o IP da
      // bridge, que não serve para aparelho nenhum.
      const address = window.location.hostname
      setupGuide.value = await apiService.get<SetupGuide>(
        `/logs/setup-snippet?address=${encodeURIComponent(address)}`
      )
    } catch {
      setupGuide.value = null
    }
  }

  return {
    filters,
    entries: list.items,
    scrollKey: list.scrollKey,
    window: list.window,
    error: list.error,
    isEmpty,
    load: list.load,
    reset: list.reset,
    prepend: list.prepend,
    applyFilters,
    clearFilters,
    tailing,
    startTail,
    stopTail,
    toggleTail,
    sources,
    unknownCount,
    sourcesLoaded,
    fetchSources,
    bindSource,
    setupGuide,
    fetchSetupGuide,
  }
})

/**
 * Cor da severidade do syslog no tema do Vuetify.
 *
 * Mora aqui, e não no componente, porque a aba de logs do dispositivo (Fase 4)
 * mostra a mesma tabela: duas tabelas de cor divergiriam na primeira alteração.
 */
export function severityColor(severity: number | null): string {
  if (severity === null) return 'grey'
  if (severity <= 2) return 'error'
  if (severity === 3) return 'error'
  if (severity === 4) return 'warning'
  if (severity <= 6) return 'info'
  return 'grey'
}
