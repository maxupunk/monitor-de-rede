import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  addMention,
  describeMention,
  findMentionQuery,
  insertMention,
  mentionsInText,
  type AiMention,
} from '@/utils/aiMentions'
import { rewindTo, toApiMessages, type AiDisplayMessage } from '@/utils/aiChatStream'
import { useMentionPicker } from '@/composables/useMentionPicker'

const mppt: AiMention = { kind: 'device', id: '12', label: 'MPPT Bateria' }
const logs: AiMention = { kind: 'source', id: 'logs', label: 'Logs' }

describe('termo do @ sob o cursor', () => {
  it('abre no início ou depois de espaço, e não no meio de um e-mail', () => {
    expect(findMentionQuery('@mp', 3)).toEqual({ start: 0, query: 'mp' })
    expect(findMentionQuery('e o @MPPT Bat', 13)).toEqual({ start: 4, query: 'MPPT Bat' })
    expect(findMentionQuery('admin@borda', 11)).toBeNull()
  })

  it('fecha quando o usuário seguiu escrevendo', () => {
    expect(findMentionQuery('@Borda ', 7)).toBeNull()
    expect(findMentionQuery('@Borda qual a tensão', 20)).toBeNull()
    expect(findMentionQuery('sem arroba', 10)).toBeNull()
  })

  it('troca o termo pelo rótulo e deixa o cursor depois do espaço', () => {
    const texto = 'e o @mp da bateria?'
    const consulta = findMentionQuery(texto, 7)!
    const resultado = insertMention(texto, consulta, 7, 'MPPT Bateria')
    expect(resultado.text).toBe('e o @MPPT Bateria da bateria?')
    expect(resultado.caret).toBe('e o @MPPT Bateria '.length)
  })
})

describe('marcações da pergunta', () => {
  it('não repete e some quando o @Rótulo sai do texto', () => {
    const lista = addMention(addMention([mppt], mppt), logs)
    expect(lista).toHaveLength(2)
    expect(mentionsInText('olhe @Logs', lista)).toEqual([logs])
  })

  it('vão para a IA com tipo e id no histórico', () => {
    expect(describeMention(mppt)).toBe('dispositivo "MPPT Bateria" (id 12)')
    expect(describeMention(logs)).toBe('fonte Logs')

    const historico = toApiMessages([
      { id: 'u1', role: 'user', content: 'tensão da @MPPT Bateria?', mentions: [mppt] },
    ])
    expect(historico[0].content).toBe(
      'tensão da @MPPT Bateria?\n[Marcados com @: dispositivo "MPPT Bateria" (id 12)]'
    )
  })

  it('a pergunta da IA entra no histórico mesmo sem texto na resposta', () => {
    const historico = toApiMessages([
      { id: 'u1', role: 'user', content: 'e a bateria?' },
      {
        id: 'a1',
        role: 'assistant',
        content: '',
        toolCalls: [
          {
            id: 'q1',
            name: 'ask_user',
            arguments: {},
            result: { question: 'De qual equipamento?', options: ['Borda', 'MPPT'] },
            status: 'done',
          },
        ],
      },
    ])
    expect(historico[1].content).toBe(
      '[Perguntei ao usuário: De qual equipamento? Opções: Borda, MPPT.]'
    )
  })
})

describe('desfazer uma pergunta', () => {
  const conversa: AiDisplayMessage[] = [
    { id: 'u1', role: 'user', content: 'dados da borda' },
    { id: 'a1', role: 'assistant', content: 'ok' },
    { id: 'u2', role: 'user', content: 'e o @MPPT Bateria?', mentions: [mppt] },
    { id: 'a2', role: 'assistant', content: 'resposta errada' },
  ]

  it('corta a pergunta e o que veio depois, devolvendo texto e marcações', () => {
    const desfeito = rewindTo(conversa, 'u2')!
    expect(desfeito.kept.map((message) => message.id)).toEqual(['u1', 'a1'])
    expect(desfeito.draft).toEqual({ content: 'e o @MPPT Bateria?', mentions: [mppt] })
  })

  it('só vale para pergunta do usuário', () => {
    expect(rewindTo(conversa, 'a1')).toBeNull()
    expect(rewindTo(conversa, 'x')).toBeNull()
  })
})

describe('lista do @', () => {
  afterEach(() => vi.useRealTimers())

  it('busca uma vez por pausa e só a última resposta vale', async () => {
    vi.useFakeTimers()
    const pendentes: Array<(value: AiMention[]) => void> = []
    const busca = vi.fn(() => new Promise<AiMention[]>((resolve) => pendentes.push(resolve)))
    const lista = useMentionPicker(busca)

    lista.update('@m', 2)
    lista.update('@mp', 3)
    await vi.advanceTimersByTimeAsync(200)
    expect(busca).toHaveBeenCalledTimes(1)
    expect(busca).toHaveBeenCalledWith('mp')

    lista.update('@mpp', 4)
    await vi.advanceTimersByTimeAsync(200)
    pendentes[1]([mppt])
    pendentes[0]([logs])
    await vi.runAllTimersAsync()
    expect(lista.options.value).toEqual([mppt])
    expect(lista.isOpen.value).toBe(true)

    lista.move(1)
    expect(lista.active.value).toEqual(mppt)
    lista.update('@mpp ', 5)
    expect(lista.isOpen.value).toBe(false)
  })
})
