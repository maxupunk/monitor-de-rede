import { ref, type Ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { PaginationMeta } from '@/bindings/PaginationMeta'

/**
 * Lista paginada consumida por `<v-infinite-scroll>`.
 *
 * O mesmo bloco de "buscar a próxima página, empilhar, decidir entre `ok` e
 * `empty`" estava copiado em cinco telas — e com ele o mesmo par de bugs
 * latentes: reiniciar a lista sem trocar a `:key` do `v-infinite-scroll` (que
 * então nunca volta a pedir a primeira página) e tratar "veio menos que o
 * limite" como fim, o que quebra quando o backend filtra linhas depois de
 * paginar.
 *
 * Uso:
 * ```ts
 * const history = useInfiniteList<MonitorResult>(() => `/monitors/${id}/results`)
 * // template: <v-infinite-scroll :key="history.scrollKey.value" @load="history.load">
 * ```
 */

/**
 * Envelope de paginação do backend (§5.4 do roadmap do backend Rust).
 *
 * `PaginationMeta` é gerado pelo `ts-rs` a partir do struct Rust
 * (`backend/src/services/shared/pagination.rs`) — se o backend mudar um
 * campo do `meta`, o `vue-tsc` acusa aqui em vez de a lista infinita parar
 * sozinha em produção.
 *
 * `meta` continua opcional porque os endpoints de modo dual devolvem array cru
 * quando `?page` está ausente; este composable sempre manda `page`, mas a
 * defesa custa nada.
 */
interface PaginatedResponse<T> {
  data: T[]
  meta?: PaginationMeta
}

type LoadStatus = 'ok' | 'empty' | 'loading' | 'error'

export interface InfiniteList<T> {
  items: Ref<T[]>
  /** Trocar esta chave remonta o `v-infinite-scroll` e reinicia a paginação */
  scrollKey: Ref<number>
  total: Ref<number>
  load: (context: { done: (status: LoadStatus) => void }) => Promise<void>
  /** Volta à primeira página e força o componente a recarregar */
  reset: () => void
}

export function useInfiniteList<T>(
  /**
   * Caminho da API sem os parâmetros de página. É uma função para a lista poder
   * seguir um alvo que muda (o `:id` da rota, um filtro selecionado) sem
   * precisar ser recriada.
   */
  endpoint: () => string,
  options: { limit?: number; label?: string } = {}
): InfiniteList<T> {
  const limit = options.limit ?? 20
  const items = ref<T[]>([]) as Ref<T[]>
  const scrollKey = ref(0)
  const total = ref(0)
  const page = ref(1)

  async function load({ done }: { done: (status: LoadStatus) => void }): Promise<void> {
    try {
      const separator = endpoint().includes('?') ? '&' : '?'
      const response = await apiService.get<PaginatedResponse<T>>(
        `${endpoint()}${separator}page=${page.value}&limit=${limit}`
      )

      const batch = Array.isArray(response.data) ? response.data : []
      if (batch.length > 0) items.value.push(...batch)
      if (response.meta) total.value = response.meta.total

      // O fim é decidido pelo `meta`, não pelo tamanho do lote: endpoints que
      // filtram linhas após paginar podem devolver uma página curta — ou vazia —
      // no meio do conjunto.
      const isLastPage = !response.meta || response.meta.currentPage >= response.meta.lastPage
      if (isLastPage) {
        done('empty')
        return
      }

      page.value++
      done('ok')
    } catch (err: unknown) {
      console.error(`Erro ao carregar ${options.label ?? endpoint()}:`, err)
      done('error')
    }
  }

  function reset(): void {
    items.value = []
    page.value = 1
    total.value = 0
    scrollKey.value++
  }

  return { items, scrollKey, total, load, reset }
}
