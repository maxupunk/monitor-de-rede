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
      color: 'grey',
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
