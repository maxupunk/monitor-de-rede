import { vi } from 'vitest'
import { apiService, ApiError } from '@/services/apiService'
import type { AiConversationSummary } from '@/bindings/AiConversationSummary'

interface StoredConversation {
  id: number
  title: string
  updatedAt: string
  messages: unknown[]
}

interface ConversationBody {
  title: string
  messages: unknown[]
}

/**
 * `/api/ai/conversations` e `/api/ai/tools/execute` em memória, ligados ao
 * `apiService` mockado. As rotas respondem pelo caminho, então a ordem em que
 * a tela dispara as chamadas não importa para o teste.
 */
export function installFakeAiApi() {
  const conversations = new Map<number, StoredConversation>()
  let nextId = 1
  let clock = 0
  const toolResponses: Array<() => unknown> = []

  const summary = (item: StoredConversation): AiConversationSummary => ({
    id: item.id,
    title: item.title,
    updatedAt: item.updatedAt,
    messageCount: item.messages.length,
  })
  const notFound = () => new ApiError('Conversa não encontrada', 404)
  const idFrom = (path: string) => Number(path.split('/').pop())
  const touch = () => new Date(Date.UTC(2026, 8, 22, 10, 0, clock++)).toISOString()

  vi.mocked(apiService.get).mockImplementation(async (path: string) => {
    if (path === '/ai/conversations') {
      return [...conversations.values()]
        .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
        .map(summary)
    }
    const found = conversations.get(idFrom(path))
    if (!found) throw notFound()
    return {
      id: found.id,
      title: found.title,
      updatedAt: found.updatedAt,
      messages: found.messages,
    }
  })

  vi.mocked(apiService.post).mockImplementation(async (path: string, body?: unknown) => {
    if (path === '/ai/tools/execute') {
      const next = toolResponses.shift()
      if (!next) throw new Error('execução não programada no teste')
      return next()
    }
    const input = body as ConversationBody
    const created = { id: nextId++, ...input, updatedAt: touch() }
    conversations.set(created.id, created)
    return summary(created)
  })

  vi.mocked(apiService.put).mockImplementation(async (path: string, body?: unknown) => {
    const found = conversations.get(idFrom(path))
    if (!found) throw notFound()
    Object.assign(found, body as ConversationBody, { updatedAt: touch() })
    return summary(found)
  })

  vi.mocked(apiService.delete).mockImplementation(async (path: string) => {
    if (!conversations.delete(idFrom(path))) throw notFound()
    return undefined
  })

  return {
    conversations,
    /** Próxima resposta de `/ai/tools/execute` (valor ou exceção). */
    onToolExecute(respond: () => unknown) {
      toolResponses.push(respond)
    },
  }
}
