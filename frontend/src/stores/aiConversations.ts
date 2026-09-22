import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService, ApiError } from '@/services/apiService'
import type { AiConversationDetail } from '@/bindings/AiConversationDetail'
import type { AiConversationInput } from '@/bindings/AiConversationInput'
import type { AiConversationSummary } from '@/bindings/AiConversationSummary'
import { useAuthStore } from './auth'
import {
  clearLocalConversations,
  conversationTitle,
  loadLocalConversations,
} from '@/utils/aiConversationStorage'

/** Mensagem mínima que a lista precisa conhecer para titular a conversa. */
interface TitledMessage {
  role: string
  content: string
}

const BASE = '/ai/conversations'

/**
 * Conversas salvas do Assistente IA, guardadas no servidor por usuário —
 * o mesmo histórico em qualquer computador. Guarda e lista; quem conversa é
 * a store `ai`, que entrega as mensagens aqui ao fim de cada resposta.
 *
 * Cada conversa aberta na tela tem uma chave local; o id do servidor é
 * associado a ela quando a primeira gravação volta. As gravações passam por
 * uma fila, então duas respostas seguidas nunca criam a conversa duas vezes.
 */
export const useAiConversationsStore = defineStore('aiConversations', () => {
  const summaries = ref<AiConversationSummary[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  let keySeq = 0
  const currentKey = ref(`draft-${keySeq}`)
  const serverIds = ref<Record<string, number>>({})
  let queue: Promise<void> = Promise.resolve()

  const activeId = computed<number | null>(() => serverIds.value[currentKey.value] ?? null)

  function userKey(): string {
    const id = useAuthStore().user?.id
    return id ? String(id) : 'anon'
  }

  function describe(err: unknown, fallback: string): string {
    if (err instanceof Error) return err.message
    return fallback
  }

  function upsertSummary(summary: AiConversationSummary) {
    summaries.value = [summary, ...summaries.value.filter((item) => item.id !== summary.id)]
  }

  /**
   * Leva para o servidor as conversas que ficaram no navegador antes do
   * histórico ser centralizado, e só então as apaga daqui.
   */
  async function migrateLocal() {
    const local = loadLocalConversations<TitledMessage>(userKey())
    if (local.length === 0) return
    // Mais antigas primeiro, para a ordem por "atualizada em" se manter.
    for (const conversation of [...local].reverse()) {
      await apiService.post<AiConversationSummary>(BASE, {
        title: conversation.title,
        messages: conversation.messages,
      } satisfies AiConversationInput)
    }
    clearLocalConversations(userKey())
  }

  /** Lista do servidor. Chamada ao abrir o histórico (ação do usuário). */
  async function load() {
    loading.value = true
    error.value = null
    try {
      await migrateLocal()
      summaries.value = await apiService.get<AiConversationSummary[]>(BASE)
    } catch (err: unknown) {
      error.value = describe(err, 'Falha ao carregar as conversas')
    } finally {
      loading.value = false
    }
  }

  /** A próxima mensagem abre uma conversa nova. */
  function startNew() {
    keySeq += 1
    currentKey.value = `draft-${keySeq}`
  }

  async function save(key: string, body: AiConversationInput) {
    const id = serverIds.value[key]
    if (id !== undefined) {
      try {
        upsertSummary(await apiService.put<AiConversationSummary>(`${BASE}/${id}`, body))
        return
      } catch (err: unknown) {
        // Apagada em outro computador: continua salvando como conversa nova.
        if (!(err instanceof ApiError) || err.status !== 404) throw err
        summaries.value = summaries.value.filter((item) => item.id !== id)
      }
    }
    const created = await apiService.post<AiConversationSummary>(BASE, body)
    serverIds.value = { ...serverIds.value, [key]: created.id }
    upsertSummary(created)
  }

  /** Grava as mensagens na conversa em tela (criando-a na primeira resposta). */
  function persist<T extends TitledMessage>(messages: T[]): Promise<void> {
    if (messages.length === 0) return queue
    const key = currentKey.value
    const body: AiConversationInput = {
      title: conversationTitle(messages.find((message) => message.role === 'user')?.content),
      messages: JSON.parse(JSON.stringify(messages)) as unknown[],
    }
    queue = queue
      .then(() => save(key, body))
      .then(() => {
        error.value = null
      })
      .catch((err: unknown) => {
        error.value = describe(err, 'Falha ao salvar a conversa')
      })
    return queue
  }

  /** Mensagens de uma conversa salva, tornando-a a conversa em tela. */
  async function open<T>(id: number): Promise<T[] | null> {
    try {
      const detail = await apiService.get<AiConversationDetail>(`${BASE}/${id}`)
      startNew()
      serverIds.value = { ...serverIds.value, [currentKey.value]: detail.id }
      return detail.messages as T[]
    } catch (err: unknown) {
      error.value = describe(err, 'Falha ao abrir a conversa')
      if (err instanceof ApiError && err.status === 404) {
        summaries.value = summaries.value.filter((item) => item.id !== id)
      }
      return null
    }
  }

  async function remove(id: number) {
    try {
      await apiService.delete(`${BASE}/${id}`)
    } catch (err: unknown) {
      if (!(err instanceof ApiError) || err.status !== 404) {
        error.value = describe(err, 'Falha ao apagar a conversa')
        return
      }
    }
    summaries.value = summaries.value.filter((item) => item.id !== id)
    if (activeId.value === id) startNew()
  }

  return {
    summaries,
    activeId,
    loading,
    error,
    load,
    startNew,
    persist,
    open,
    remove,
  }
})
