/**
 * Regras puras do chat da IA: como cada evento do stream muda a mensagem em
 * construção e como as mensagens viram o histórico enviado ao backend.
 */
import type { AiChart } from '@/bindings/AiChart'
import type { ContextHint } from '@/bindings/ContextHint'
import { describeMention, type AiMention } from './aiMentions'

export type AiToolStatus = 'running' | 'done' | 'error' | 'awaiting' | 'cancelled'

export interface AiToolCallState {
  id: string
  name: string
  arguments: Record<string, unknown>
  result?: Record<string, unknown> | null
  /** Gráfico desenhado no chat; a IA recebe só o resumo em `result`. */
  chart?: AiChart | null
  /** Frase do pedido de confirmação ("Silenciar o alerta #12 por 60 min"). */
  summary?: string | null
  status: AiToolStatus
}

export interface AiUsageInfo {
  promptTokens: number
  completionTokens: number
  /** Modelo que de fato respondeu (um roteador escolhe por pergunta). */
  model?: string | null
  /** Tamanho da conversa na última rodada: o que a próxima pergunta carrega. */
  contextTokens?: number
  /** Janela do modelo, em tokens. */
  contextWindow?: number | null
  /** A janela veio do provedor (`true`) ou é estimada pelo nome do modelo. */
  contextWindowReported?: boolean
  /** Tempo do provedor escrevendo — base do tokens/s. */
  generationMs?: number
  /** Tempo total da resposta, ferramentas incluídas. */
  durationMs?: number
  /** Parte da entrada servida do cache do provedor. */
  cachedTokens?: number
  /** Grupos de ferramentas carregados — voltam na próxima pergunta. */
  toolGroups?: string[]
}

/** Registro de uma compactação: as mensagens até esta viraram `summary`. */
export interface AiCompaction {
  summary: string
  tokensBefore: number
  tokensAfter: number
  /** `false` quando o provedor não resumiu e as mensagens só saíram. */
  summarized: boolean
}

export interface AiDisplayMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  /** Presente na última mensagem coberta por uma compactação do contexto. */
  compaction?: AiCompaction | null
  toolCalls?: AiToolCallState[]
  /** O que o usuário marcou com `@` nesta pergunta. */
  mentions?: AiMention[]
  usage?: AiUsageInfo | null
  isStreaming?: boolean
  error?: string | null
}

export interface ApiChatMessage {
  role: 'user' | 'assistant'
  content: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function asRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {}
}

/** Número positivo do evento, ou `undefined` quando o backend não o mandou. */
function positive(value: unknown): number | undefined {
  const number = Number(value)
  return Number.isFinite(number) && number > 0 ? number : undefined
}

/** Métricas da resposta; só entra o que o evento trouxe. */
function usageFrom(event: Record<string, unknown>): AiUsageInfo {
  const usage: AiUsageInfo = {
    promptTokens: positive(event.promptTokens) ?? 0,
    completionTokens: positive(event.completionTokens) ?? 0,
  }
  if (typeof event.model === 'string' && event.model) usage.model = event.model
  const contextTokens = positive(event.contextTokens)
  if (contextTokens !== undefined) usage.contextTokens = contextTokens
  const contextWindow = positive(event.contextWindow)
  if (contextWindow !== undefined) usage.contextWindow = contextWindow
  const generationMs = positive(event.generationMs)
  if (generationMs !== undefined) usage.generationMs = generationMs
  const durationMs = positive(event.durationMs)
  if (durationMs !== undefined) usage.durationMs = durationMs
  if (typeof event.contextWindowReported === 'boolean') {
    usage.contextWindowReported = event.contextWindowReported
  }
  const cachedTokens = positive(event.cachedTokens)
  if (cachedTokens !== undefined) usage.cachedTokens = cachedTokens
  if (Array.isArray(event.toolGroups)) {
    usage.toolGroups = event.toolGroups.filter(
      (group): group is string => typeof group === 'string'
    )
  }
  return usage
}

/** Aplica um evento do stream à mensagem da IA em construção. */
export function applyChatEvent(message: AiDisplayMessage, event: unknown): void {
  if (!isRecord(event)) return
  const tools = (message.toolCalls ??= [])
  const findTool = () => tools.find((tool) => tool.id === event.id)

  switch (event.type) {
    case 'textDelta':
      if (typeof event.content === 'string') message.content += event.content
      break
    case 'toolCall':
      tools.push({
        id: String(event.id),
        name: String(event.name),
        arguments: asRecord(event.arguments),
        status: 'running',
      })
      break
    case 'toolResult': {
      const tool = findTool()
      if (!tool) break
      const result = asRecord(event.result)
      tool.result = result
      tool.chart = (event.chart as AiChart | undefined) ?? null
      tool.status = result.error ? 'error' : 'done'
      break
    }
    case 'confirmationRequired':
      tools.push({
        id: String(event.id),
        name: String(event.name),
        arguments: asRecord(event.arguments),
        summary: typeof event.summary === 'string' ? event.summary : null,
        status: 'awaiting',
      })
      break
    case 'usage':
      message.usage = usageFrom(event)
      break
    case 'error':
      message.error = typeof event.message === 'string' ? event.message : 'Erro no provedor'
      break
    case 'done':
      message.isStreaming = false
      break
  }
}

/**
 * Janela de escrita curta demais para medir velocidade: o provedor entregou
 * tudo num pedaço só, e dividir por ~0 daria milhares de tokens/s.
 */
const MIN_GENERATION_MS = 250

/** Velocidade de geração em tokens/s; `null` sem tokens ou sem tempo medido. */
export function tokensPerSecond(usage?: AiUsageInfo | null): number | null {
  if (!usage?.completionTokens) return null
  const generation = usage.generationMs ?? 0
  const elapsed = generation >= MIN_GENERATION_MS ? generation : (usage.durationMs ?? 0)
  if (elapsed <= 0) return null
  return usage.completionTokens / (elapsed / 1000)
}

/** Nome curto do modelo: sem o prefixo do provedor (`meta-llama/`). */
export function shortModelName(model?: string | null): string | null {
  if (!model) return null
  const name = model.slice(model.lastIndexOf('/') + 1)
  return name || model
}

export interface AiContextUsage {
  used: number
  window: number
  /** 0–1. */
  ratio: number
  /** A janela veio do provedor; `false` quando é estimada. */
  reported: boolean
}

/**
 * Quanto da janela a conversa ocupa, pela resposta mais recente que trouxe
 * a medida. `null` antes da primeira resposta medida.
 */
export function contextUsage(messages: AiDisplayMessage[]): AiContextUsage | null {
  for (let index = messages.length - 1; index >= 0; index--) {
    const usage = messages[index].usage
    if (usage?.contextTokens && usage.contextWindow) {
      return {
        used: usage.contextTokens,
        window: usage.contextWindow,
        ratio: Math.min(1, usage.contextTokens / usage.contextWindow),
        reported: usage.contextWindowReported === true,
      }
    }
  }
  return null
}

/** Uso da janela a partir do qual o backend compacta a conversa sozinho. */
export const AUTO_COMPACT_RATIO = 0.8

/** O que o chat manda ao backend além da pergunta. */
export interface AiChatHistory {
  messages: ApiChatMessage[]
  /** Id da mensagem de origem de cada item de `messages`, na mesma ordem. */
  ids: string[]
  /** Resumo das mensagens já compactadas, que não vão mais em `messages`. */
  summary: string | null
  contextHint: ContextHint | null
}

/**
 * Histórico a enviar: o resumo da última compactação mais tudo o que veio
 * depois dela, e a medida da resposta anterior para o backend decidir se
 * compacta de novo.
 */
export function buildChatHistory(messages: AiDisplayMessage[]): AiChatHistory {
  let start = 0
  let summary: string | null = null
  for (let index = messages.length - 1; index >= 0; index--) {
    const compaction = messages[index].compaction
    if (compaction) {
      start = index + 1
      summary = compaction.summary
      break
    }
  }
  const entries = toApiEntries(messages.slice(start))

  let contextHint: ContextHint | null = null
  for (let index = messages.length - 1; index >= 0; index--) {
    const usage = messages[index].usage
    if (usage?.contextTokens) {
      contextHint = {
        tokens: usage.contextTokens,
        window: usage.contextWindow ?? null,
        model: usage.model ?? null,
        toolGroups: usage.toolGroups ?? [],
      }
      break
    }
  }
  return {
    messages: entries.map((entry) => entry.message),
    ids: entries.map((entry) => entry.id),
    summary,
    contextHint,
  }
}

/**
 * Evento `contextCompacted`: marca a última mensagem que o resumo cobre.
 * `ids` é a lista enviada no pedido — o backend conta a partir dela.
 */
export function applyCompaction(
  messages: AiDisplayMessage[],
  ids: string[],
  event: Record<string, unknown>
): void {
  const folded = Number(event.foldedMessages)
  if (!Number.isInteger(folded) || folded < 1 || folded > ids.length) return
  if (typeof event.summary !== 'string' || !event.summary.trim()) return
  const target = messages.find((message) => message.id === ids[folded - 1])
  if (!target) return
  target.compaction = {
    summary: event.summary,
    tokensBefore: Number(event.tokensBefore) || 0,
    tokensAfter: Number(event.tokensAfter) || 0,
    summarized: event.summarized !== false,
  }
}

/** Nome da ferramenta com que a IA pergunta ao usuário antes de prosseguir. */
export const ASK_USER_TOOL = 'ask_user'

export interface AiQuestion {
  question: string
  options: string[]
}

/** A pergunta que a IA fez com `ask_user`, quando o resultado a trouxe. */
export function toolQuestion(tool: AiToolCallState): AiQuestion | null {
  if (tool.name !== ASK_USER_TOOL || typeof tool.result?.question !== 'string') return null
  const options = Array.isArray(tool.result.options)
    ? tool.result.options.filter((option): option is string => typeof option === 'string')
    : []
  return { question: tool.result.question, options }
}

/**
 * Nota que conta à IA o que ficou fora do texto: o que o usuário decidiu
 * sobre uma ação proposta, ou a pergunta que ela mesma fez.
 */
function decisionNote(tool: AiToolCallState): string | null {
  const question = toolQuestion(tool)
  if (question) {
    const options = question.options.length ? ` Opções: ${question.options.join(', ')}.` : ''
    return `[Perguntei ao usuário: ${question.question}${options}]`
  }
  const label = tool.summary || tool.name
  if (tool.status === 'cancelled') return `[Ação cancelada pelo usuário: ${label}]`
  if (tool.status === 'awaiting') return `[Ação ainda aguardando confirmação: ${label}]`
  if (tool.summary && tool.status === 'done') return `[Ação confirmada e executada: ${label}]`
  if (tool.summary && tool.status === 'error') return `[Ação confirmada, mas falhou: ${label}]`
  return null
}

/**
 * Histórico enviado ao backend. Mensagens com erro ficam de fora; as
 * respostas levam anotadas as decisões sobre ações propostas, para a IA não
 * propor de novo o que já foi feito ou recusado.
 */
export function toApiMessages(messages: AiDisplayMessage[]): ApiChatMessage[] {
  return toApiEntries(messages).map((entry) => entry.message)
}

/** Cada mensagem enviada junto do id da mensagem da tela de onde saiu. */
function toApiEntries(messages: AiDisplayMessage[]): { id: string; message: ApiChatMessage }[] {
  return messages
    .filter((message) => !message.error)
    .map((message) => {
      const notes = (message.toolCalls ?? [])
        .map(decisionNote)
        .filter((note): note is string => note !== null)
      if (message.mentions?.length) {
        notes.push(`[Marcados com @: ${message.mentions.map(describeMention).join('; ')}]`)
      }
      const content = [message.content, ...notes].filter((part) => part.trim()).join('\n')
      return { id: message.id, message: { role: message.role, content } }
    })
    .filter((entry) => entry.message.content.trim().length > 0)
}

/** O que o "desfazer" devolve ao campo: a pergunta e o que estava marcado. */
export interface AiDraft {
  content: string
  mentions: AiMention[]
}

/**
 * Volta a conversa para antes da pergunta `messageId`: ela e tudo o que veio
 * depois saem, e o texto dela volta para o campo. `null` quando o id não é
 * de uma pergunta do usuário.
 */
export function rewindTo(
  messages: AiDisplayMessage[],
  messageId: string
): { kept: AiDisplayMessage[]; draft: AiDraft } | null {
  const index = messages.findIndex((message) => message.id === messageId)
  if (index < 0 || messages[index].role !== 'user') return null
  const question = messages[index]
  return {
    kept: messages.slice(0, index),
    draft: { content: question.content, mentions: question.mentions ?? [] },
  }
}
