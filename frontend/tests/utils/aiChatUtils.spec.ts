import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { stubBrowserStorage } from '../helpers/storage'
import {
  clearLocalConversations,
  conversationTitle,
  loadLocalConversations,
} from '@/utils/aiConversationStorage'
import { readSseJson } from '@/utils/sseReader'
import {
  applyChatEvent,
  buildChatHistory,
  contextUsage,
  INTERRUPTED_TOOL,
  interruptRunningTools,
  shortModelName,
  tokensPerSecond,
  type AiDisplayMessage,
} from '@/utils/aiChatStream'
import { formatCompactCount, formatElapsedMs, formatTokenRate } from '@/utils/formatters'

function readerOf(...chunks: string[]): ReadableStreamDefaultReader<Uint8Array> {
  const encoder = new TextEncoder()
  return new ReadableStream<Uint8Array>({
    start(controller) {
      chunks.forEach((chunk) => controller.enqueue(encoder.encode(chunk)))
      controller.close()
    },
  }).getReader()
}

describe('conversas do navegador', () => {
  beforeEach(() => stubBrowserStorage())

  it('título corta numa palavra inteira', () => {
    expect(conversationTitle(undefined)).toBe('Nova conversa')
    expect(conversationTitle('  ping   no gateway ')).toBe('ping no gateway')
    const longo = conversationTitle(
      'Quais interfaces do roteador de borda estão caídas ou saturadas desde ontem à noite?'
    )
    expect(longo.endsWith('…')).toBe(true)
    expect(longo.length).toBeLessThanOrEqual(61)
  })

  it('conversas antigas do navegador: JSON inválido vira lista vazia e a limpeza remove', () => {
    localStorage.setItem('netmonitor.ai.conversations.1', '{quebrado')
    expect(loadLocalConversations('1')).toEqual([])
    localStorage.setItem('netmonitor.ai.conversations.1', JSON.stringify([{ id: 1 }]))
    expect(loadLocalConversations('1')).toEqual([])

    const valida = [{ id: 'c', title: 't', updatedAt: '2026-09-21T10:00:00Z', messages: [] }]
    localStorage.setItem('netmonitor.ai.conversations.1', JSON.stringify(valida))
    expect(loadLocalConversations('1')).toEqual(valida)
    expect(loadLocalConversations('2')).toEqual([])
    clearLocalConversations('1')
    expect(loadLocalConversations('1')).toEqual([])
  })
})

describe('leitor de SSE', () => {
  it('junta eventos quebrados entre pedaços e ignora keep-alive', async () => {
    const eventos: unknown[] = []
    await readSseJson(
      readerOf(
        'data: {"type":"textDe',
        'lta","content":"oi"}\n\n: keep-alive\n',
        'data: {"type":"done"}'
      ),
      (event) => eventos.push(event)
    )
    expect(eventos).toEqual([{ type: 'textDelta', content: 'oi' }, { type: 'done' }])
  })
})

describe('eventos do chat', () => {
  function resposta(): AiDisplayMessage {
    return { id: 'a', role: 'assistant', content: '', toolCalls: [], isStreaming: true }
  }

  it('resposta retirada pela conferência some e a conferida toma o lugar', () => {
    const msg = resposta()
    applyChatEvent(msg, { type: 'textDelta', content: 'No `agente-west-01`.' })
    applyChatEvent(msg, { type: 'notice', message: 'Conferindo a resposta: agente-west-01' })
    applyChatEvent(msg, { type: 'retract' })
    applyChatEvent(msg, { type: 'textDelta', content: 'Não há host com esse nome.' })
    expect(msg.content).toBe('Não há host com esse nome.')
    expect(msg.notices?.[0].message).toContain('Conferindo')
  })

  it('retirada apaga só a última rodada, nunca o que veio antes das ferramentas', () => {
    const msg = resposta()
    applyChatEvent(msg, { type: 'textDelta', content: 'Vou consultar os hosts. ' })
    applyChatEvent(msg, { type: 'toolCall', id: 't', name: 'get_docker_hosts', arguments: {} })
    applyChatEvent(msg, { type: 'toolResult', id: 't', result: { hosts: [] } })
    applyChatEvent(msg, { type: 'textDelta', content: 'No `agente-west-01`.' })
    applyChatEvent(msg, { type: 'retract' })
    applyChatEvent(msg, { type: 'textDelta', content: 'Sim, no `agente-west-01`.' })
    // A conferida ainda inventou: troca de novo, no mesmo ponto.
    applyChatEvent(msg, { type: 'retract' })
    applyChatEvent(msg, { type: 'textDelta', content: 'Sim, no `[não confirmado]`.' })
    expect(msg.content).toBe('Vou consultar os hosts. Sim, no `[não confirmado]`.')
  })

  it('avisos se acumulam e só os de configuração levam ao ajuste', () => {
    const msg = resposta()
    applyChatEvent(msg, { type: 'notice', message: 'Janela de 4096 tokens', settings: true })
    applyChatEvent(msg, { type: 'notice', message: 'A IA vai acessar um equipamento real' })
    applyChatEvent(msg, { type: 'notice', message: 'Janela de 4096 tokens', settings: true })
    expect(msg.notices).toEqual([
      { message: 'Janela de 4096 tokens', settings: true },
      { message: 'A IA vai acessar um equipamento real', settings: false },
    ])
  })

  it('aviso da conferência troca de estado no lugar, sem empilhar', () => {
    const msg = resposta()
    applyChatEvent(msg, { type: 'notice', message: 'A IA vai acessar um equipamento real' })
    applyChatEvent(msg, { type: 'notice', message: 'Conferindo a resposta…', key: 'answer-check' })
    applyChatEvent(msg, { type: 'notice', message: 'Resposta conferida.', key: 'answer-check' })
    expect(msg.notices?.map((notice) => notice.message)).toEqual([
      'A IA vai acessar um equipamento real',
      'Resposta conferida.',
    ])
  })

  it('resultado com erro marca a ferramenta como falha', () => {
    const msg = resposta()
    applyChatEvent(msg, { type: 'toolCall', id: 't', name: 'ping_host', arguments: {} })
    applyChatEvent(msg, { type: 'toolResult', id: 't', result: { error: 'timeout' } })
    expect(msg.toolCalls?.[0].status).toBe('error')
    applyChatEvent(msg, 'lixo')
    applyChatEvent(msg, { type: 'done' })
    expect(msg.isStreaming).toBe(false)
  })

  it('aviso do backend fica na mensagem sem virar erro', () => {
    const msg = resposta()
    applyChatEvent(msg, { type: 'notice', message: 'Janela de 4096 tokens' })
    expect(msg.notices?.[0].message).toBe('Janela de 4096 tokens')
    expect(msg.error).toBeUndefined()
  })

  it('histórico deixa de fora mensagens com erro e vazias', () => {
    const mensagens: AiDisplayMessage[] = [
      { id: '1', role: 'user', content: 'oi' },
      { id: '2', role: 'assistant', content: '', error: 'falhou' },
      { id: '3', role: 'assistant', content: '' },
    ]
    expect(buildChatHistory(mensagens).messages).toEqual([{ role: 'user', content: 'oi' }])
  })

  it('decisão sobre uma ação volta à IA só com a primeira linha do resumo', () => {
    const mensagens: AiDisplayMessage[] = [
      { id: '1', role: 'user', content: 'reinicie o roteador' },
      {
        id: '2',
        role: 'assistant',
        content: 'Proponho reiniciar.',
        toolCalls: [
          {
            id: 't1',
            name: 'device_ssh_exec',
            arguments: {},
            summary: 'Executar reboot em Borda\nATENÇÃO: ALTERA O EQUIPAMENTO\nmotivo: travado',
            status: 'cancelled',
          },
        ],
      },
    ]
    expect(buildChatHistory(mensagens).messages[1].content).toBe(
      'Proponho reiniciar.\n[Ação cancelada pelo usuário: Executar reboot em Borda]'
    )
  })

  it('resposta com erro perde o texto mas mantém a ação que já rodou', () => {
    const mensagens: AiDisplayMessage[] = [
      { id: '1', role: 'user', content: 'reinicia o ollama' },
      {
        id: '2',
        role: 'assistant',
        content: 'Reiniciando…',
        error: 'timeout',
        toolCalls: [
          {
            id: 't1',
            name: 'docker_container_action',
            arguments: {},
            status: 'done',
            risk: 'low',
          },
        ],
      },
    ]
    expect(buildChatHistory(mensagens).messages[1].content).toBe(
      '[Ação executada automaticamente: docker_container_action]'
    )
  })

  it('ação confirmada leva o resultado resumido para a IA', () => {
    const mensagens: AiDisplayMessage[] = [
      { id: '1', role: 'user', content: 'pinga o gateway' },
      {
        id: '2',
        role: 'assistant',
        content: '',
        toolCalls: [
          {
            id: 't1',
            name: 'ping',
            arguments: {},
            summary: 'Ping em 10.0.0.1',
            status: 'done',
            result: { loss_pct: 0, avg_ms: 2.5 },
          },
        ],
      },
    ]
    expect(buildChatHistory(mensagens).messages[1].content).toBe(
      '[Ação confirmada e executada: Ping em 10.0.0.1. Resultado: {"loss_pct":0,"avg_ms":2.5}]'
    )
  })

  it('ferramenta sem resultado vira falha com o motivo quando a resposta para', () => {
    const msg: AiDisplayMessage = {
      id: '2',
      role: 'assistant',
      content: '',
      toolCalls: [
        { id: 't1', name: 'get_alerts', arguments: {}, status: 'running' },
        { id: 't2', name: 'ping', arguments: {}, status: 'done', result: { ok: true } },
      ],
    }
    interruptRunningTools(msg, INTERRUPTED_TOOL)
    expect(msg.toolCalls?.[0]).toMatchObject({
      status: 'error',
      result: { error: INTERRUPTED_TOOL },
    })
    expect(msg.toolCalls?.[1].status).toBe('done')
  })
})

describe('contagem compacta', () => {
  it('formata tokens', () => {
    expect(formatCompactCount(850)).toBe('850')
    expect(formatCompactCount(1234)).toBe('1,2 mil')
    expect(formatCompactCount(3_400_000)).toBe('3,4 mi')
    expect(formatCompactCount(null)).toBe('—')
  })
})

describe('métricas da resposta', () => {
  it('evento de uso traz modelo, contexto e tempos', () => {
    const msg: AiDisplayMessage = { id: 'a', role: 'assistant', content: '' }
    applyChatEvent(msg, {
      type: 'usage',
      promptTokens: 1200,
      completionTokens: 80,
      model: 'meta-llama/llama-3.3-70b-instruct',
      contextTokens: 950,
      contextWindow: 131072,
      generationMs: 2000,
      durationMs: 5000,
    })
    expect(msg.usage).toEqual({
      promptTokens: 1200,
      completionTokens: 80,
      model: 'meta-llama/llama-3.3-70b-instruct',
      contextTokens: 950,
      contextWindow: 131072,
      generationMs: 2000,
      durationMs: 5000,
    })
    expect(tokensPerSecond(msg.usage)).toBe(40)
    expect(shortModelName(msg.usage?.model)).toBe('llama-3.3-70b-instruct')
  })

  it('tokens/s cai no tempo total quando a escrita foi curta demais para medir', () => {
    expect(
      tokensPerSecond({ promptTokens: 1, completionTokens: 50, generationMs: 10, durationMs: 1000 })
    ).toBe(50)
    expect(tokensPerSecond({ promptTokens: 1, completionTokens: 0, generationMs: 900 })).toBeNull()
    expect(tokensPerSecond({ promptTokens: 1, completionTokens: 9 })).toBeNull()
    expect(tokensPerSecond(null)).toBeNull()
  })

  it('contexto vem da resposta mais recente que o mediu', () => {
    const mensagens: AiDisplayMessage[] = [
      {
        id: '1',
        role: 'assistant',
        content: 'a',
        usage: { promptTokens: 1, completionTokens: 1, contextTokens: 1000, contextWindow: 8000 },
      },
      { id: '2', role: 'user', content: 'b' },
      { id: '3', role: 'assistant', content: 'c', usage: { promptTokens: 1, completionTokens: 1 } },
    ]
    expect(contextUsage(mensagens)).toEqual({
      used: 1000,
      window: 8000,
      ratio: 0.125,
      reported: false,
    })
    expect(contextUsage([])).toBeNull()
  })

  it('formata velocidade e duração', () => {
    expect(formatTokenRate(42.34)).toBe('42,3 tok/s')
    expect(formatTokenRate(120.6)).toBe('121 tok/s')
    expect(formatTokenRate(null)).toBe('—')
    expect(formatElapsedMs(850)).toBe('850 ms')
    expect(formatElapsedMs(4230)).toBe('4,2 s')
    expect(formatElapsedMs(119_600)).toBe('2 min 0 s')
  })
})

afterEach(() => {
  vi.unstubAllGlobals()
})
