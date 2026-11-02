import type { AiSettings } from '@/stores/ai'
import type { AiProactiveSettings } from '@/bindings/AiProactiveSettings'
import { formatContextWindow } from '@/utils/formatters'

/**
 * Fonte única dos padrões dos provedores de IA: endpoint, modelo, janela de
 * contexto e a normalização que o formulário aplica ao carregar, testar e
 * salvar. Antes, cada um desses valores aparecia repetido no cartão.
 */

export type AiDriver = 'opencode' | 'openrouter' | 'ollama'

/** Endpoint fixo do OpenCode Zen (não há campo para trocá-lo). */
export const OPENCODE_BASE_URL = 'https://opencode.ai/zen/v1'

export const OLLAMA_DEFAULT_BASE_URL = 'http://ollama:11434/v1'

export const DEFAULT_OLLAMA_NUM_CTX = 16384

/** O que muda no chat com um modelo treinado com as ferramentas do NetMonitor. */
export const NETMONITOR_TRAINED_HINT =
  'Envia o prompt enxuto direto ao modelo, sem o roteamento de ferramentas do Ollaya. ' +
  'O Ollaya continua verificando a segurança dos comandos e pedindo a sua confirmação quando necessário.'

/** Modelo usado quando o provedor está configurado sem modelo escolhido. */
export const DEFAULT_MODELS: Record<AiDriver, string> = {
  opencode: 'muse-spark-1.3-contributor-free',
  openrouter: 'openrouter/free',
  ollama: 'lfm3-netmonitor:latest',
}

export const DRIVER_LABELS: Record<AiDriver, string> = {
  opencode: 'OpenCode Go / Zen',
  openrouter: 'OpenRouter Gateway',
  ollama: 'Ollama Local',
}

export const DRIVER_OPTIONS: { title: string; value: AiDriver }[] = [
  { title: 'Ollama (local, nesta rede)', value: 'ollama' },
  { title: 'OpenRouter (vários modelos na nuvem)', value: 'openrouter' },
  { title: 'OpenCode Go / Zen', value: 'opencode' },
]

/** Degraus da janela do Ollama e o que cada um significa para quem escolhe. */
const OLLAMA_NUM_CTX_STEPS: ReadonlyMap<number, string> = new Map([
  [8192, 'econômico'],
  [DEFAULT_OLLAMA_NUM_CTX, 'recomendado'],
  [32768, 'médio'],
  [65536, 'amplo'],
  [131072, 'estendido'],
  [262144, 'máximo'],
])

export interface NumCtxOption {
  title: string
  value: number
}

function positiveTokens(value: number | null | undefined): number | null {
  return value && Number.isFinite(value) && value > 0 ? value : null
}

/**
 * A janela que o campo assume ao escolher um modelo: a nativa dele (tabela
 * de recomendados ou `/api/show` do instalado) ou, sem essa informação, a
 * recomendada — a mesma `OLLAMA_CONTEXT_LENGTH` padrão do servidor.
 */
export function ollamaNumCtxFor(modelWindow: number | null | undefined): number {
  return positiveTokens(modelWindow) ?? DEFAULT_OLLAMA_NUM_CTX
}

function numCtxTag(value: number, modelWindow: number | null): string | undefined {
  if (modelWindow !== null && value === modelWindow) {
    return value === DEFAULT_OLLAMA_NUM_CTX ? 'recomendado · máximo do modelo' : 'máximo do modelo'
  }
  if (modelWindow !== null && value > modelWindow) return 'acima do máximo do modelo'
  return OLLAMA_NUM_CTX_STEPS.get(value)
}

/**
 * Opções do campo `num_ctx` para o modelo escolhido: os degraus que cabem na
 * janela dele, a própria janela (ex.: 40k do `qwen3`) e o valor atual, para
 * ele nunca sumir da lista. Sem janela conhecida, todos os degraus.
 */
export function ollamaNumCtxOptions(
  modelWindow: number | null | undefined,
  current: number | null | undefined
): NumCtxOption[] {
  const max = positiveTokens(modelWindow)
  const values = new Set([...OLLAMA_NUM_CTX_STEPS.keys()].filter((v) => max === null || v <= max))
  if (max !== null) values.add(max)
  const selected = positiveTokens(current)
  if (selected !== null) values.add(selected)
  return [...values]
    .sort((a, b) => a - b)
    .map((value) => {
      const tag = numCtxTag(value, max)
      const label = formatContextWindow(value, true)
      return { value, title: tag ? `${label} — ${tag}` : label }
    })
}

/** Antigo padrão gratuito do OpenRouter, retirado do ar: vira o roteador gratuito. */
const RETIRED_OPENROUTER_MODEL = 'meta-llama/llama-3.3-70b-instruct:free'

const DEFAULT_MAX_SUMMARIES_PER_HOUR = 6

export function defaultProactive(): AiProactiveSettings {
  return {
    incidentSummaries: false,
    incidentMinSeverity: 'critical',
    maxSummariesPerHour: DEFAULT_MAX_SUMMARIES_PER_HOUR,
    digest: 'off',
    digestHour: 8,
    skipQuietDigest: false,
  }
}

export function defaultAiSettings(): AiSettings {
  return {
    enabled: false,
    activeDriver: 'ollama',
    opencodeBaseUrl: OPENCODE_BASE_URL,
    opencodeApiKey: '',
    opencodeModel: DEFAULT_MODELS.opencode,
    openrouterApiKey: '',
    openrouterModel: DEFAULT_MODELS.openrouter,
    ollamaBaseUrl: OLLAMA_DEFAULT_BASE_URL,
    ollamaModel: DEFAULT_MODELS.ollama,
    ollamaNumCtx: DEFAULT_OLLAMA_NUM_CTX,
    ollamaReasoning: false,
    ollamaNetmonitorTrained: null,
    allowActiveTools: true,
    requireToolConfirmation: false,
    actionMode: 'off',
    containerActions: 'confirm',
    responseStyle: 'concise',
    proactive: defaultProactive(),
    customSystemPrompt: '',
  }
}

/** O driver gravado é texto livre no contrato; desconhecido cai no Ollama. */
export function resolveDriver(driver: string | null | undefined): AiDriver {
  return driver === 'openrouter' || driver === 'opencode' ? driver : 'ollama'
}

/** Slug puro do modelo, venha ele como texto ou como item de combobox (`id`/`name`/`value`). */
export function extractModelId(value: unknown): string {
  if (!value) return ''
  if (typeof value === 'string') return value.trim()
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>
    for (const key of ['id', 'name', 'value']) {
      const candidate = record[key]
      if (typeof candidate === 'string' && candidate.trim()) return candidate.trim()
    }
  }
  return String(value).trim()
}

function openrouterModel(model: unknown): string {
  const id = extractModelId(model)
  return !id || id === RETIRED_OPENROUTER_MODEL ? DEFAULT_MODELS.openrouter : id
}

/**
 * Configuração pronta para a tela e para a API: slugs limpos, endpoint do
 * OpenCode, migração do modelo retirado do OpenRouter, contexto do Ollama e
 * o limite de resumos (campo numérico apagado chega como `''`).
 * Devolve uma cópia — `proactive` incluído — sem tocar na entrada.
 */
export function normalizeAiSettings(settings: AiSettings): AiSettings {
  const proactive = { ...defaultProactive(), ...settings.proactive }
  proactive.maxSummariesPerHour =
    Number(proactive.maxSummariesPerHour) || DEFAULT_MAX_SUMMARIES_PER_HOUR
  return {
    ...settings,
    opencodeBaseUrl: settings.opencodeBaseUrl || OPENCODE_BASE_URL,
    opencodeModel: extractModelId(settings.opencodeModel),
    openrouterModel: openrouterModel(settings.openrouterModel),
    ollamaModel: extractModelId(settings.ollamaModel),
    ollamaNumCtx: settings.ollamaNumCtx || DEFAULT_OLLAMA_NUM_CTX,
    ollamaReasoning: settings.ollamaReasoning ?? false,
    ollamaNetmonitorTrained: settings.ollamaNetmonitorTrained ?? null,
    proactive,
  }
}

export interface ConnectionTestInput {
  driver: AiDriver
  baseUrl: string | null
  apiKey: string | null
  model: string
}

/** O que o botão "Testar conexão" envia para o provedor ativo. */
export function connectionTestInput(settings: AiSettings): ConnectionTestInput {
  const driver = resolveDriver(settings.activeDriver)
  if (driver === 'opencode') {
    return {
      driver,
      baseUrl: settings.opencodeBaseUrl || OPENCODE_BASE_URL,
      apiKey: settings.opencodeApiKey || null,
      model: extractModelId(settings.opencodeModel) || DEFAULT_MODELS.opencode,
    }
  }
  if (driver === 'openrouter') {
    return {
      driver,
      baseUrl: null,
      apiKey: settings.openrouterApiKey || null,
      model: openrouterModel(settings.openrouterModel),
    }
  }
  return {
    driver,
    baseUrl: settings.ollamaBaseUrl || OLLAMA_DEFAULT_BASE_URL,
    apiKey: null,
    model: extractModelId(settings.ollamaModel) || DEFAULT_MODELS.ollama,
  }
}

/** Campo do formulário que guarda o modelo de cada provedor. */
export const MODEL_FIELDS = {
  opencode: 'opencodeModel',
  openrouter: 'openrouterModel',
  ollama: 'ollamaModel',
} as const satisfies Record<AiDriver, keyof AiSettings>

/** Modelo escolhido para o provedor ativo. */
export function modelForDriver(settings: AiSettings): string {
  return settings[MODEL_FIELDS[resolveDriver(settings.activeDriver)]] || ''
}
