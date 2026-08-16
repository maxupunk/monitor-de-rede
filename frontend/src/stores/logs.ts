import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { useInfiniteCursor } from '@/composables/useInfiniteCursor'
import type { LogEntry } from '@/bindings/LogEntry'

export type { LogEntry }

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
