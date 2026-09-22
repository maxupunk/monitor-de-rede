import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { stubBrowserStorage } from '../helpers/storage'
import {
  clearLocalConversations,
  conversationTitle,
  loadLocalConversations,
} from '@/utils/aiConversationStorage'
import { readSseJson } from '@/utils/sseReader'
import { applyChatEvent, toApiMessages, type AiDisplayMessage } from '@/utils/aiChatStream'
import { formatCompactCount } from '@/utils/formatters'

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

afterEach(() => {
  vi.unstubAllGlobals()
})
