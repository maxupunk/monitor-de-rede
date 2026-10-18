import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAiStore, type AiChart } from '@/stores/ai'
import { apiService } from '@/services/apiService'
import { aiToolMeta, formatToolArgs } from '@/components/ai/aiToolMeta'
import { responseStyleOption } from '@/components/ai/aiResponseStyle'

vi.mock('@/services/apiService', () => ({
  apiService: {
    get: vi.fn(),
    put: vi.fn(),
    post: vi.fn(),
    postStream: vi.fn(),
  },
  ApiError: class ApiError extends Error {},
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

const chart: AiChart = {
  title: 'Latência — Ping Borda',
  subtitle: 'última hora',
  unit: 'latency',
  series: [
    {
      id: 'monitor-1',
      label: 'Latência',
      points: [{ time: '2026-09-21T10:00:00+00:00', value: 12.5 }],
    },
  ],
  avgValue: 12.5,
}

describe('chat da IA', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('guarda o gráfico do resultado da ferramenta junto da chamada', async () => {
    vi.mocked(apiService.postStream).mockResolvedValueOnce(
      sseResponse([
        {
          type: 'toolCall',
          id: 'c1',
          name: 'chart_monitor_latency',
          arguments: { monitor_id: 1 },
        },
        {
          type: 'toolResult',
          id: 'c1',
          name: 'chart_monitor_latency',
          result: { chart_shown: true },
          chart,
        },
        { type: 'textDelta', content: 'Latência estável.' },
        { type: 'done' },
      ])
    )

    const store = useAiStore()
    await store.sendMessage('Mostre a latência do gateway')

    const resposta = store.messages[1]
    expect(resposta.content).toBe('Latência estável.')
    expect(resposta.toolCalls?.[0].status).toBe('done')
    expect(resposta.toolCalls?.[0].chart).toEqual(chart)
  })

  it('ferramenta sem gráfico fica com chart nulo', async () => {
    vi.mocked(apiService.postStream).mockResolvedValueOnce(
      sseResponse([
        { type: 'toolCall', id: 'c1', name: 'get_alerts', arguments: {} },
        { type: 'toolResult', id: 'c1', name: 'get_alerts', result: { total: 0 } },
        { type: 'done' },
      ])
    )

    const store = useAiStore()
    await store.sendMessage('Há alertas?')

    expect(store.messages[1].toolCalls?.[0].chart).toBeNull()
  })
})

describe('apresentação das ferramentas e do estilo', () => {
  it('ferramenta desconhecida ganha rótulo legível', () => {
    expect(aiToolMeta('chart_interface_traffic').label).toBe('Gráfico de Tráfego')
    expect(aiToolMeta('ferramenta_nova')).toEqual({
      label: 'ferramenta_nova',
      icon: 'mdi-cog-outline',
      color: 'primary',
      kind: 'action',
    })
  })

  it('resume os argumentos pelo que identifica a chamada', () => {
    expect(formatToolArgs({ device: 'Borda', interface: 'ether1', hours: 6 })).toBe(
      'Dispositivo: Borda · Interface: ether1'
    )
    expect(formatToolArgs({ pattern: 'link down', source: 'logs', output: 'count' })).toBe(
      'Padrão: link down · Fonte: logs'
    )
    expect(aiToolMeta('grep').label).toBe('Busca nos Dados (grep)')
    expect(formatToolArgs({ hours: 6 })).toBe('hours')
    expect(formatToolArgs({})).toBe('Sem parâmetros adicionais')
  })

  it('estilo ausente cai no modo direto', () => {
    expect(responseStyleOption(undefined).value).toBe('concise')
    expect(responseStyleOption('normal').title).toBe('Normal')
  })
})

describe('acesso salvo no cartão da ferramenta', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    vi.mocked(apiService.get).mockResolvedValue({
      deviceId: 7,
      deviceName: 'AP-terraço',
      credentials: [],
      agents: [],
    })
  })

  function pendingStream() {
    let finish: (response: Response) => void = () => undefined
    vi.mocked(apiService.postStream).mockImplementationOnce(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve
        })
    )
    return (events: unknown[] = [{ type: 'done' }]) => finish(sseResponse(events))
  }

  it('no meio de uma resposta, a mensagem espera ela terminar e segue na mesma conversa', async () => {
    const store = useAiStore()
    const finish = pendingStream()
    vi.mocked(apiService.postStream).mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    const first = store.sendMessage('Quais canais o AP-terraço usa?')
    expect(store.isStreaming).toBe(true)

    await expect(store.continueAfterAccess({ deviceId: 7, transport: 'ssh' })).resolves.toBe(
      'queued'
    )
    expect(apiService.get).toHaveBeenCalledWith('/devices/7/access')
    expect(apiService.postStream).toHaveBeenCalledTimes(1)

    finish()
    await first
    await vi.waitFor(() => expect(apiService.postStream).toHaveBeenCalledTimes(2))
    expect(
      store.messages.filter((message) => message.role === 'user').map((message) => message.content)
    ).toEqual([
      'Quais canais o AP-terraço usa?',
      'Cadastrei o acesso SSH de AP-terraço; pode tentar de novo.',
    ])
  })

  it('trocar de conversa descarta a mensagem que esperava', async () => {
    const store = useAiStore()
    const finish = pendingStream()
    const first = store.sendMessage('Quais canais o AP-terraço usa?')
    await expect(store.continueAfterAccess({ deviceId: 7, transport: 'ssh' })).resolves.toBe(
      'queued'
    )
    store.newConversation()
    finish()
    await first
    expect(apiService.postStream).toHaveBeenCalledTimes(1)
    expect(store.messages).toEqual([])
  })

  it('sem o nome do equipamento, a mensagem ainda diz o que foi cadastrado', async () => {
    vi.mocked(apiService.get).mockRejectedValueOnce(new Error('offline'))
    vi.mocked(apiService.postStream).mockResolvedValueOnce(sseResponse([{ type: 'done' }]))
    const store = useAiStore()
    await expect(store.continueAfterAccess({ deviceId: 7, transport: 'telnet' })).resolves.toBe(
      'sent'
    )
    expect(store.messages[0]?.content).toBe(
      'Cadastrei o acesso Telnet do equipamento; pode tentar de novo.'
    )
  })
})
