import { ref, type Ref } from 'vue'
import { apiService } from '@/services/apiService'

/**
 * Lista infinita paginada por **cursor**, irmã de `useInfiniteList`.
 *
 * Por que não reaproveitar o `useInfiniteList`: ele decide o fim da lista por
 * `meta.currentPage >= meta.lastPage`, e um cursor não tem `lastPage`. Nem
 * poderia ter — contar as linhas da janela custaria um `COUNT(*)` a cada
 * rolagem sobre milhões de registros no banco de logs.
 *
 * E o motivo de o backend paginar por cursor, e não por `OFFSET`, é o mesmo que
 * impede fabricar número de página falso aqui: a tabela de logs recebe
 * inserção o tempo todo, e com `OFFSET` cada mensagem nova desloca a janela
 * inteira — a rolagem repetiria e pularia linhas.
 *
 * Uso:
 * ```ts
 * const logs = useInfiniteCursor<LogEntry>(() => `/logs?severity=3`)
 * // template: <v-infinite-scroll :key="logs.scrollKey.value" @load="logs.load">
 * ```
 */

interface CursorMeta {
  nextCursor: string | null
  hasMore: boolean
  limit: number
  from: string
  to: string
}

interface CursorResponse<T> {
  data: T[]
  meta: CursorMeta
}

type LoadStatus = 'ok' | 'empty' | 'loading' | 'error'

export interface InfiniteCursor<T> {
  items: Ref<T[]>
  /** Trocar esta chave remonta o `v-infinite-scroll` e reinicia a paginação */
  scrollKey: Ref<number>
  /** Janela efetivamente consultada pelo backend, já com o teto aplicado */
  window: Ref<{ from: string; to: string } | null>
  error: Ref<string | null>
  load: (context: { done: (status: LoadStatus) => void }) => Promise<void>
  /** Volta ao começo e força o componente a recarregar */
  reset: () => void
  /** Insere no topo, para o live tail da Fase 4 */
  prepend: (entries: T[], keyOf: (entry: T) => string | number) => void
}

export function useInfiniteCursor<T>(
  /**
   * Caminho da API sem `cursor`. É uma função para a lista poder seguir
   * filtros que mudam sem precisar ser recriada.
   */
  endpoint: () => string,
  options: { limit?: number; label?: string; max?: number } = {}
): InfiniteCursor<T> {
  const limit = options.limit ?? 50
  /** Teto do que fica em memória: o live tail empilha sem parar */
  const max = options.max ?? 5000

  const items = ref<T[]>([]) as Ref<T[]>
  const scrollKey = ref(0)
  const window = ref<{ from: string; to: string } | null>(null)
  const error = ref<string | null>(null)
  const cursor = ref<string | null>(null)
  /**
   * A rolagem infinita chama `load` de novo assim que a lista cresce; sem esta
   * trava, uma resposta lenta viraria duas requisições com o mesmo cursor e as
   * mesmas linhas apareceriam duas vezes.
   */
  let generation = 0
  let loadingGeneration: number | null = null

  async function load({ done }: { done: (status: LoadStatus) => void }): Promise<void> {
    const requestGeneration = generation
    if (loadingGeneration === requestGeneration) {
      done('ok')
      return
    }
    loadingGeneration = requestGeneration
    try {
      const currentEndpoint = endpoint()
      const separator = currentEndpoint.includes('?') ? '&' : '?'
      const currentCursor = cursor.value
      const query = currentCursor ? `&cursor=${encodeURIComponent(currentCursor)}` : ''
      const response = await apiService.get<CursorResponse<T>>(
        `${currentEndpoint}${separator}limit=${limit}${query}`
      )

      // Um reset pode trocar o dispositivo enquanto a requisição anterior
      // ainda está em voo. A resposta antiga nunca pode contaminar o novo
      // escopo da lista.
      if (requestGeneration !== generation) {
        done('ok')
        return
      }

      const batch = Array.isArray(response.data) ? response.data : []
      if (batch.length > 0) items.value.push(...batch)
      if (response.meta) {
        window.value = { from: response.meta.from, to: response.meta.to }
      }
      error.value = null

      // O fim é `nextCursor` nulo, não lote curto: o backend pode devolver
      // página incompleta e ainda ter mais.
      const next = response.meta?.nextCursor ?? null
      cursor.value = next
      done(next ? 'ok' : 'empty')
    } catch (err: unknown) {
      if (requestGeneration !== generation) {
        done('ok')
        return
      }
      error.value = err instanceof Error ? err.message : 'Falha ao carregar os registros.'
      console.error(`Erro ao carregar ${options.label ?? endpoint()}:`, err)
      done('error')
    } finally {
      if (loadingGeneration === requestGeneration) loadingGeneration = null
    }
  }

  function reset(): void {
    generation++
    loadingGeneration = null
    items.value = []
    cursor.value = null
    window.value = null
    error.value = null
    scrollKey.value++
  }

  /**
   * Empilha no topo o que chegou pelo tempo real, sem duplicar o que a
   * paginação já trouxe — as duas fontes se sobrepõem na fronteira.
   */
  function prepend(entries: T[], keyOf: (entry: T) => string | number): void {
    if (entries.length === 0) return
    const conhecidos = new Set(items.value.map(keyOf))
    const novos = entries.filter((entry) => !conhecidos.has(keyOf(entry)))
    if (novos.length === 0) return
    items.value.unshift(...novos)
    if (items.value.length > max) items.value.length = max
  }

  return { items, scrollKey, window, error, load, reset, prepend }
}
