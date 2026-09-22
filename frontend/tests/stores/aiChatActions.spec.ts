import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { stubBrowserStorage } from '../helpers/storage'
import { installFakeAiApi } from '../helpers/fakeAiApi'
import { createPinia, setActivePinia } from 'pinia'
import { useAiStore } from '@/stores/ai'
import { useAiConversationsStore } from '@/stores/aiConversations'
import { apiService } from '@/services/apiService'

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

const proposta = {
  type: 'confirmationRequired',
  id: 'c1',
  name: 'silence_alert',
  arguments: { alert_id: 7, minutes: 30 },
  summary: 'Silenciar o alerta #7 por 30 min',
}

async function conversaComProposta() {
  vi.mocked(apiService.postStream).mockResolvedValueOnce(
    sseResponse([
      proposta,
      { type: 'textDelta', content: 'Propus silenciar o alerta.' },
      { type: 'usage', promptTokens: 1500, completionTokens: 40 },
      { type: 'done' },
    ])
  )
  const store = useAiStore()
  await store.sendMessage('Silencie o alerta 7')
  return store
}

let api: ReturnType<typeof installFakeAiApi>

beforeEach(() => {
  stubBrowserStorage()
  setActivePinia(createPinia())
  vi.clearAllMocks()
  api = installFakeAiApi()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('ações propostas pela IA', () => {
  it('a proposta chega aguardando confirmação e o uso de tokens fica na resposta', async () => {
    const store = await conversaComProposta()
    const resposta = store.messages[1]
    expect(resposta.toolCalls?.[0]).toMatchObject({
      name: 'silence_alert',
      status: 'awaiting',
      summary: 'Silenciar o alerta #7 por 30 min',
    })
    expect(resposta.usage).toEqual({ promptTokens: 1500, completionTokens: 40 })
    expect(apiService.post).not.toHaveBeenCalledWith('/ai/tools/execute', expect.anything())
  })

  it('confirmar executa em nome do usuário e grava o resultado', async () => {
    const store = await conversaComProposta()
    api.onToolExecute(() => ({ result: { done: true, status: 'silenced' }, chart: null }))

    await store.confirmTool(store.messages[1].id, 'c1')

    expect(apiService.post).toHaveBeenCalledWith('/ai/tools/execute', {
      name: 'silence_alert',
      arguments: { alert_id: 7, minutes: 30 },
    })
    expect(store.messages[1].toolCalls?.[0].status).toBe('done')
  })

  it('falha na execução vira erro visível, sem travar o card', async () => {
    const store = await conversaComProposta()
    api.onToolExecute(() => {
      throw new Error('Sem permissão')
    })

    await store.confirmTool(store.messages[1].id, 'c1')

    const tool = store.messages[1].toolCalls?.[0]
    expect(tool?.status).toBe('error')
    expect(tool?.result).toEqual({ error: 'Sem permissão' })
  })

  it('a decisão do usuário vai anotada no histórico da próxima pergunta', async () => {
    const store = await conversaComProposta()
    store.cancelTool(store.messages[1].id, 'c1')

    vi.mocked(apiService.postStream).mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    await store.sendMessage('ok')

    const corpo = vi.mocked(apiService.postStream).mock.calls[1][1] as {
      messages: { role: string; content: string }[]
    }
    expect(corpo.messages[1].content).toContain(
      '[Ação cancelada pelo usuário: Silenciar o alerta #7 por 30 min]'
    )
  })
})

describe('desfazer e marcar com @', () => {
  const mppt = { kind: 'device' as const, id: '12', label: 'MPPT Bateria' }

  it('a marcação vai no corpo e fica anotada no histórico seguinte', async () => {
    vi.mocked(apiService.postStream)
      .mockResolvedValueOnce(
        sseResponse([{ type: 'textDelta', content: '48 V' }, { type: 'done' }])
      )
      .mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    const store = useAiStore()
    await store.sendMessage('tensão da @MPPT Bateria?', {}, [mppt])
    await store.sendMessage('e ontem?')

    const [primeiro, segundo] = vi
      .mocked(apiService.postStream)
      .mock.calls.map((call) => call[1] as { mentions: unknown[]; messages: { content: string }[] })
    expect(primeiro.mentions).toEqual([mppt])
    expect(segundo.mentions).toEqual([])
    expect(segundo.messages[0].content).toContain('[Marcados com @: dispositivo "MPPT Bateria"')
  })

  it('desfazer volta a conversa para antes da pergunta e a salva assim', async () => {
    vi.mocked(apiService.postStream)
      .mockResolvedValueOnce(
        sseResponse([{ type: 'textDelta', content: 'borda ok' }, { type: 'done' }])
      )
      .mockResolvedValueOnce(
        sseResponse([{ type: 'textDelta', content: 'errado' }, { type: 'done' }])
      )
    const store = useAiStore()
    await store.sendMessage('dados da borda')
    await store.sendMessage('e a @MPPT Bateria?', {}, [mppt])
    const conversas = useAiConversationsStore()
    await vi.waitFor(() => expect(conversas.activeId).not.toBeNull())

    const rascunho = store.rewind(store.messages[2].id)

    expect(rascunho).toEqual({ content: 'e a @MPPT Bateria?', mentions: [mppt] })
    expect(store.messages.map((message) => message.content)).toEqual(['dados da borda', 'borda ok'])
    await vi.waitFor(() =>
      expect(api.conversations.get(conversas.activeId!)?.messages).toHaveLength(2)
    )
    expect(store.rewind(store.messages[1].id)).toBeNull()
  })

  it('desfazer durante a resposta interrompe a geração', async () => {
    vi.mocked(apiService.postStream).mockImplementationOnce(
      (_path, _body, signal) =>
        new Promise((_resolve, reject) => {
          signal?.addEventListener('abort', () =>
            reject(Object.assign(new Error('abort'), { name: 'AbortError' }))
          )
        })
    )
    const store = useAiStore()
    const envio = store.sendMessage('pergunta longa')
    expect(store.isStreaming).toBe(true)

    store.rewind(store.messages[0].id)
    await envio

    expect(store.isStreaming).toBe(false)
    expect(store.messages).toHaveLength(0)
  })
})

describe('conversas salvas na conta', () => {
  it('a conversa vai para o servidor e reabre depois de começar outra', async () => {
    const store = await conversaComProposta()
    const conversas = useAiConversationsStore()
    await vi.waitFor(() => expect(conversas.activeId).not.toBeNull())
    const id = conversas.activeId!
    expect(api.conversations.get(id)?.title).toBe('Silencie o alerta 7')

    store.newConversation()
    expect(store.messages).toHaveLength(0)

    await store.openConversation(id)
    expect(store.messages).toHaveLength(2)
    expect(store.messages[1].toolCalls?.[0].status).toBe('awaiting')

    await store.deleteConversation(id)
    expect(api.conversations.size).toBe(0)
    expect(store.messages).toHaveLength(0)
  })

  it('respostas seguidas atualizam a mesma conversa, sem duplicar', async () => {
    const store = await conversaComProposta()
    vi.mocked(apiService.postStream).mockResolvedValueOnce(
      sseResponse([{ type: 'textDelta', content: 'feito' }, { type: 'done' }])
    )
    await store.sendMessage('e agora?')
    store.cancelTool(store.messages[1].id, 'c1')
    await useAiConversationsStore().persist(store.messages)

    expect(api.conversations.size).toBe(1)
    const [salva] = api.conversations.values()
    expect(salva.messages).toHaveLength(4)
  })

  it('conversa apagada em outro computador é recriada ao continuar', async () => {
    const store = await conversaComProposta()
    const conversas = useAiConversationsStore()
    await vi.waitFor(() => expect(conversas.activeId).not.toBeNull())
    api.conversations.clear()

    vi.mocked(apiService.postStream).mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    await store.sendMessage('continua')
    await conversas.persist(store.messages)

    expect(api.conversations.size).toBe(1)
    expect(conversas.error).toBeNull()
  })

  it('conversas antigas do navegador sobem para a conta e saem do navegador', async () => {
    localStorage.setItem(
      'netmonitor.ai.conversations.anon',
      JSON.stringify([
        {
          id: 'conv-2',
          title: 'Mais nova',
          updatedAt: '2026-09-21T11:00:00Z',
          messages: [{ id: 'u', role: 'user', content: 'b' }],
        },
        {
          id: 'conv-1',
          title: 'Mais antiga',
          updatedAt: '2026-09-21T10:00:00Z',
          messages: [{ id: 'u', role: 'user', content: 'a' }],
        },
      ])
    )
    const conversas = useAiConversationsStore()
    await conversas.load()

    expect(conversas.summaries.map((item) => item.title)).toEqual(['Mais nova', 'Mais antiga'])
    expect(localStorage.getItem('netmonitor.ai.conversations.anon')).toBeNull()
  })

  it('"Diagnosticar com IA" abre o painel numa conversa nova com o contexto da tela', async () => {
    vi.mocked(apiService.postStream).mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    const store = useAiStore()

    expect(store.askAbout('Investigue o alerta #3', { alertId: 3, deviceId: 9 })).toBe(true)
    await vi.waitFor(() => expect(apiService.postStream).toHaveBeenCalled())

    expect(store.isDrawerOpen).toBe(true)
    const corpo = vi.mocked(apiService.postStream).mock.calls[0][1] as Record<string, unknown>
    expect(corpo).toMatchObject({ alertId: 3, deviceId: 9 })
    expect(corpo.monitorId).toBeUndefined()
  })
})
