/**
 * Regras puras do chat da IA: como cada evento do stream muda a mensagem em
 * construção e como as mensagens viram o histórico enviado ao backend.
 */
import type { AiChart } from '@/bindings/AiChart'

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

/** Nota que conta à IA o que o usuário decidiu sobre uma ação proposta. */
function decisionNote(tool: AiToolCallState): string | null {
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
      const content = [message.content, ...notes].filter((part) => part.trim()).join('\n')
      return { role: message.role, content }
    })
    .filter((message) => message.content.trim().length > 0)
}
