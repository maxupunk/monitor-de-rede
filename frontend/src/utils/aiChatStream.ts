/**
 * Regras puras do chat da IA: como cada evento do stream muda a mensagem em
 * construção e como as mensagens viram o histórico enviado ao backend.
 */
import type { AiChart } from '@/bindings/AiChart'
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
}

export interface AiDisplayMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
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
      message.usage = {
        promptTokens: Number(event.promptTokens) || 0,
        completionTokens: Number(event.completionTokens) || 0,
      }
      break
    case 'error':
      message.error = typeof event.message === 'string' ? event.message : 'Erro no provedor'
      break
    case 'done':
      message.isStreaming = false
      break
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
      return { role: message.role, content }
    })
    .filter((message) => message.content.trim().length > 0)
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
