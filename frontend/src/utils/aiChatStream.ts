/**
 * Regras puras do chat da IA: como cada evento do stream muda a mensagem em
 * construção e como as mensagens viram o histórico enviado ao backend.
 */
import type { AiRating } from '@/utils/aiRating'
import type { AiChart } from '@/bindings/AiChart'
import type { ContextHint } from '@/bindings/ContextHint'
import { describeMention, type AiMention } from './aiMentions'
import { isAiToolRisk, type AiToolRisk } from './aiRisk'

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
  /** Acesso a equipamento liberado pelo modo "Aceitar automaticamente". */
  autoApproved?: boolean
  /**
   * Risco que o backend viu: no pedido de confirmação, por que pergunta; numa
   * ação que o modo automático rodou sozinho, por que não perguntou.
   */
  risk?: AiToolRisk
  reason?: string
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

/** Aviso do backend sobre a resposta. */
export interface AiNotice {
  message: string
  /** Resolve-se nas configurações da IA: a tela oferece o atalho. */
  settings: boolean
  /** Aviso que muda de estado ("conferindo" → "conferida"): o novo com a mesma chave toma o lugar do anterior. */
  key?: string
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
  /** A tela em que o usuário estava ao perguntar. */
  screen?: { id: string; label: string; icon: string } | null
  /** A avaliação de quem conversou (só nas respostas). */
  rating?: AiRating | null
  usage?: AiUsageInfo | null
  isStreaming?: boolean
  error?: string | null
  /** Avisos sobre a resposta (janela pequena, conferência, equipamento real). */
  notices?: AiNotice[]
  /**
   * Onde começa o texto da rodada atual (depois da última ferramenta): o
   * `retract` da conferência apaga só dali em diante.
   */
  roundStart?: number
}

export interface ApiChatMessage {
  role: 'user' | 'assistant'
  content: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** Risco e motivo do evento, quando vierem. */
function riskFrom(event: Record<string, unknown>): Pick<AiToolCallState, 'risk' | 'reason'> {
  if (!isAiToolRisk(event.risk)) return {}
  return {
    risk: event.risk,
    ...(typeof event.reason === 'string' && event.reason ? { reason: event.reason } : {}),
  }
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

/**
 * Ferramentas que ficaram "rodando" quando a resposta parou (o usuário
 * interrompeu, a conexão caiu, a conversa foi reaberta) viram falha com o
 * motivo — o resultado nunca chegou, e o cartão não pode girar para sempre.
 */
export function interruptRunningTools(message: AiDisplayMessage, reason: string): void {
  for (const tool of message.toolCalls ?? []) {
    if (tool.status !== 'running') continue
    tool.status = 'error'
    tool.result = { error: reason }
  }
}

/** Motivo de uma ferramenta sem resultado: ela pode ter rodado no servidor. */
export const INTERRUPTED_TOOL =
  'Interrompido antes do resultado; a ação pode ter sido executada no servidor'

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
      message.roundStart = message.content.length
      tools.push({
        id: String(event.id),
        name: String(event.name),
        arguments: asRecord(event.arguments),
        status: 'running',
        ...(event.autoApproved === true ? { autoApproved: true } : {}),
        ...riskFrom(event),
      })
      break
    case 'toolResult': {
      message.roundStart = message.content.length
      const tool = findTool()
      if (!tool) break
      const result = asRecord(event.result)
      tool.result = result
      tool.chart = (event.chart as AiChart | undefined) ?? null
      tool.status = result.error ? 'error' : 'done'
      break
    }
    case 'confirmationRequired':
      message.roundStart = message.content.length
      tools.push({
        id: String(event.id),
        name: String(event.name),
        arguments: asRecord(event.arguments),
        summary: typeof event.summary === 'string' ? event.summary : null,
        status: 'awaiting',
        ...riskFrom(event),
      })
      break
    case 'usage':
      message.usage = usageFrom(event)
      break
    case 'error':
      message.error = typeof event.message === 'string' ? event.message : 'Erro no provedor'
      break
    case 'notice': {
      if (typeof event.message !== 'string' || !event.message) break
      const notices = (message.notices ??= [])
      const key = typeof event.key === 'string' && event.key ? event.key : undefined
      const notice: AiNotice = { message: event.message, settings: event.settings === true, key }
      const previous = key ? notices.findIndex((item) => item.key === key) : -1
      if (previous >= 0) notices.splice(previous, 1, notice)
      else if (!notices.some((item) => item.message === event.message)) notices.push(notice)
      break
    }
    case 'retract':
      // A conferência do harness descartou o texto da última rodada (citava
      // o que nenhuma consulta mostrou): o que veio antes das ferramentas fica,
      // e a versão conferida vem em seguida.
      message.content = message.content.slice(0, message.roundStart ?? 0)
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
const ASK_USER_TOOL = 'ask_user'

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

/** Teto do rótulo de uma decisão: o resumo pode ter várias linhas e avisos. */
const MAX_DECISION_LABEL = 120
/** Caracteres do resultado de uma ação que voltam para a IA na nota. */
const MAX_RESULT_EXCERPT = 400

/**
 * O que identifica a ação na nota: a primeira linha do resumo, curta. O resto
 * (avisos de acesso a equipamento, detalhes) já foi lido pelo usuário e só
 * custaria tokens em toda pergunta seguinte.
 */
function decisionLabel(tool: AiToolCallState): string {
  const first = (tool.summary ?? '').split('\n')[0].trim()
  if (!first) return tool.name
  return first.length > MAX_DECISION_LABEL ? `${first.slice(0, MAX_DECISION_LABEL)}…` : first
}

/**
 * O resultado de uma ação, curto: a execução confirmada pelo cartão acontece
 * fora do stream, e sem ele a IA não sabe o que a ação devolveu (o ping que o
 * usuário confirmou, o erro do equipamento).
 */
function resultExcerpt(tool: AiToolCallState): string {
  if (!tool.result) return ''
  const text = JSON.stringify(tool.result)
  const short = text.length > MAX_RESULT_EXCERPT ? `${text.slice(0, MAX_RESULT_EXCERPT)}…` : text
  return ` Resultado: ${short}`
}

/**
 * Nota que conta à IA o que ficou fora do texto: o que o usuário decidiu
 * sobre uma ação proposta, a ação que o modo automático rodou sozinho, ou a
 * pergunta que ela mesma fez.
 */
function decisionNote(tool: AiToolCallState): string | null {
  const question = toolQuestion(tool)
  if (question) {
    const options = question.options.length ? ` Opções: ${question.options.join(', ')}.` : ''
    return `[Perguntei ao usuário: ${question.question}${options}]`
  }
  const label = decisionLabel(tool)
  if (tool.status === 'cancelled') return `[Ação cancelada pelo usuário: ${label}]`
  if (tool.status === 'awaiting') return `[Ação ainda aguardando confirmação: ${label}]`
  if (tool.summary && tool.status === 'done') {
    return `[Ação confirmada e executada: ${label}.${resultExcerpt(tool)}]`
  }
  if (tool.summary && tool.status === 'error') {
    return `[Ação confirmada, mas falhou: ${label}.${resultExcerpt(tool)}]`
  }
  // Sem pedido de confirmação: o modo automático (ou o "Aceitar
  // automaticamente" do equipamento) rodou a ação sozinho.
  if ((tool.risk || tool.autoApproved) && tool.status === 'done') {
    return `[Ação executada automaticamente: ${label}]`
  }
  if ((tool.risk || tool.autoApproved) && tool.status === 'error') {
    return `[Ação automática falhou ou foi interrompida: ${label}]`
  }
  return null
}

/**
 * Cada mensagem enviada junto do id da mensagem da tela de onde saiu.
 * Mensagens com erro perdem o texto (incompleto) mas não as notas: a ação que
 * rodou antes do erro aconteceu. As respostas levam anotadas as decisões
 * sobre ações propostas, para a IA não propor de novo o que já foi feito ou
 * recusado. As marcações da última pergunta não viram nota: vão em `mentions`
 * e o backend as expande no contexto da vez.
 */
function toApiEntries(messages: AiDisplayMessage[]): { id: string; message: ApiChatMessage }[] {
  const last = messages[messages.length - 1]
  return messages
    .map((message) => {
      const notes = (message.toolCalls ?? [])
        .map(decisionNote)
        .filter((note): note is string => note !== null)
      if (message.mentions?.length && message !== last) {
        notes.push(`[Marcados com @: ${message.mentions.map(describeMention).join('; ')}]`)
      }
      const text = message.error ? '' : message.content
      const content = [text, ...notes].filter((part) => part.trim()).join('\n')
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
