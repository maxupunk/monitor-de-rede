import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { stubBrowserStorage } from '../helpers/storage'
import { installFakeAiApi } from '../helpers/fakeAiApi'
import { useAiStore } from '@/stores/ai'
import { apiService } from '@/services/apiService'
import {
  applyCompaction,
  buildChatHistory,
  contextUsage,
  type AiDisplayMessage,
} from '@/utils/aiChatStream'

vi.mock('@/services/apiService', () => ({
  apiService: {
    get: vi.fn(),
    put: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
    postStream: vi.fn(),
  },
  ApiError: class ApiError extends Error {
    constructor(
      message: string,
      readonly status: number
    ) {
      super(message)
    }
  },
}))

function sseResponse(events: unknown[]): Response {
  const body = events.map((event) => `data: ${JSON.stringify(event)}\n\n`).join('')
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(body))
      controller.close()
    },
  })
  return new Response(stream)
}

function sentBody(call: number): Record<string, unknown> {
  return vi.mocked(apiService.postStream).mock.calls[call][1] as Record<string, unknown>
}

beforeEach(() => {
  stubBrowserStorage()
  setActivePinia(createPinia())
  vi.clearAllMocks()
  installFakeAiApi()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('histórico enviado ao backend', () => {
  const conversa = (): AiDisplayMessage[] => [
    { id: 'u1', role: 'user', content: 'status da borda' },
    {
      id: 'a1',
      role: 'assistant',
      content: 'perda de 20%',
      usage: {
        promptTokens: 900,
        completionTokens: 50,
        contextTokens: 950,
        contextWindow: 8000,
        contextWindowReported: true,
        model: 'meta/llama',
      },
    },
    { id: 'u2', role: 'user', content: 'e a filial?' },
    { id: 'a2', role: 'assistant', content: '', error: 'falhou' },
    { id: 'u3', role: 'user', content: 'tenta de novo' },
  ]

  it('sem compactação vai tudo, com a medida da última resposta', () => {
    const historico = buildChatHistory(conversa())
    expect(historico.ids).toEqual(['u1', 'a1', 'u2', 'u3'])
    expect(historico.summary).toBeNull()
    expect(historico.contextHint).toEqual({ tokens: 950, window: 8000, model: 'meta/llama' })
    expect(contextUsage(conversa())?.reported).toBe(true)
  })

  it('a compactação marca a última mensagem coberta e o envio passa a começar depois dela', () => {
    const mensagens = conversa()
    const enviado = buildChatHistory(mensagens)
    applyCompaction(mensagens, enviado.ids, {
      type: 'contextCompacted',
      summary: '- borda com perda de 20%',
      foldedMessages: 3,
      tokensBefore: 7000,
      tokensAfter: 2000,
      summarized: true,
    })
    expect(mensagens.find((m) => m.id === 'u2')?.compaction).toEqual({
      summary: '- borda com perda de 20%',
      tokensBefore: 7000,
      tokensAfter: 2000,
      summarized: true,
    })

    const depois = buildChatHistory(mensagens)
    expect(depois.summary).toBe('- borda com perda de 20%')
    expect(depois.ids).toEqual(['u3'])
  })

  it('evento inválido não marca nada', () => {
    const mensagens = conversa()
    const ids = buildChatHistory(mensagens).ids
    applyCompaction(mensagens, ids, { foldedMessages: 99, summary: 'x' })
    applyCompaction(mensagens, ids, { foldedMessages: 1, summary: '  ' })
    applyCompaction(mensagens, ids, { foldedMessages: 0, summary: 'x' })
    expect(mensagens.some((m) => m.compaction)).toBe(false)
  })
})

describe('compactação no chat', () => {
  it('depois de compactar, a próxima pergunta leva o resumo e só o que veio depois', async () => {
    const store = useAiStore()
    vi.mocked(apiService.postStream).mockResolvedValueOnce(
      sseResponse([
        { type: 'textDelta', content: 'A borda perdeu 20%.' },
        { type: 'usage', promptTokens: 7000, completionTokens: 20, contextTokens: 7020 },
        { type: 'done' },
      ])
    )
    await store.sendMessage('status da borda')

    vi.mocked(apiService.postStream).mockResolvedValueOnce(
      sseResponse([
        {
          type: 'contextCompacted',
          summary: '- borda com perda de 20%',
          foldedMessages: 2,
          tokensBefore: 7100,
          tokensAfter: 1500,
          summarized: true,
        },
        { type: 'textDelta', content: 'A filial está no ar.' },
        { type: 'done' },
      ])
    )
    await store.sendMessage('e a filial?')
    expect(sentBody(1)).toMatchObject({ summary: null, compact: false })
    expect(sentBody(1).contextHint).toMatchObject({ tokens: 7020 })
    expect(store.messages[1].compaction?.summary).toBe('- borda com perda de 20%')

    vi.mocked(apiService.postStream).mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    store.requestCompaction()
    await store.sendMessage('e o gateway?')
    const terceiro = sentBody(2)
    expect(terceiro.summary).toBe('- borda com perda de 20%')
    expect(terceiro.compact).toBe(true)
    expect(terceiro.messages).toEqual([
      { role: 'user', content: 'e a filial?' },
      { role: 'assistant', content: 'A filial está no ar.' },
      { role: 'user', content: 'e o gateway?' },
    ])
    expect(store.compactNext).toBe(false)
  })
})
