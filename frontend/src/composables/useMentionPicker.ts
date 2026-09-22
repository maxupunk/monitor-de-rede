import { computed, getCurrentInstance, onBeforeUnmount, ref } from 'vue'
import { findMentionQuery, type AiMention, type MentionQuery } from '@/utils/aiMentions'

/** Espera entre teclas antes de buscar: uma busca por pausa, não por tecla. */
const SEARCH_DELAY_MS = 150

/**
 * Lista do `@` do campo do chat: acompanha o termo sob o cursor, busca as
 * sugestões (só a resposta da última busca vale) e guarda a seleção do
 * teclado. Quem busca é injetado — a store na tela, um falso no teste.
 */
export function useMentionPicker(search: (query: string) => Promise<AiMention[]>) {
  const query = ref<MentionQuery | null>(null)
  const options = ref<AiMention[]>([])
  const activeIndex = ref(0)
  const loading = ref(false)

  let timer: ReturnType<typeof setTimeout> | undefined
  let lastRequest = 0

  const isOpen = computed(() => query.value !== null && (loading.value || options.value.length > 0))

  function close() {
    if (timer) clearTimeout(timer)
    timer = undefined
    lastRequest += 1
    query.value = null
    options.value = []
    loading.value = false
  }

  async function fetch(term: string) {
    const request = ++lastRequest
    loading.value = true
    try {
      const found = await search(term)
      if (request !== lastRequest) return
      options.value = found
      activeIndex.value = 0
    } catch {
      if (request === lastRequest) options.value = []
    } finally {
      if (request === lastRequest) loading.value = false
    }
  }

  /** Recalcula a partir do texto e da posição do cursor. */
  function update(text: string, caret: number) {
    const next = findMentionQuery(text, caret)
    if (!next) {
      close()
      return
    }
    const changed = next.query !== query.value?.query
    query.value = next
    if (!changed) return
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => void fetch(next.query), SEARCH_DELAY_MS)
  }

  function move(delta: number) {
    const count = options.value.length
    if (count === 0) return
    activeIndex.value = (activeIndex.value + delta + count) % count
  }

  const active = computed<AiMention | null>(() => options.value[activeIndex.value] ?? null)

  if (getCurrentInstance()) onBeforeUnmount(close)

  return { query, options, activeIndex, active, loading, isOpen, update, move, close }
}
