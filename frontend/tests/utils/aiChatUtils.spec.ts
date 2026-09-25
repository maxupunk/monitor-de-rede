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
  contextUsage,
  shortModelName,
  toApiMessages,
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
    expect(msg.notice).toBe('Janela de 4096 tokens')
    expect(msg.error).toBeUndefined()
  })

  it('histórico deixa de fora mensagens com erro e vazias', () => {
    const mensagens: AiDisplayMessage[] = [
      { id: '1', role: 'user', content: 'oi' },
      { id: '2', role: 'assistant', content: '', error: 'falhou' },
      { id: '3', role: 'assistant', content: '' },
    ]
    expect(toApiMessages(mensagens)).toEqual([{ role: 'user', content: 'oi' }])
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
