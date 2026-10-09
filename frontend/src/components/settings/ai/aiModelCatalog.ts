import type { FilterFunction } from 'vuetify'
import type { OpenCodeModelItem } from '@/bindings/OpenCodeModelItem'
import type { OpenRouterModelItem } from '@/bindings/OpenRouterModelItem'
import type { OllamaModelItem, OllamaRecommendedModel } from '@/stores/ai'
import { isModelInstalled } from '@/stores/aiLaya'
import { formatCompactCount, formatDecimalBytes } from '@/utils/formatters'

/**
 * Forma única de um modelo na tela, qualquer que seja o provedor: o seletor
 * do formulário, o instalador do Ollama e o catálogo completo usam a mesma
 * lista e os mesmos selos (`AiModelChips`).
 */
export interface AiModelChoice {
  id: string
  name: string
  description: string | null
  isFree: boolean
  supportsTools: boolean
  /** Contexto em tokens, quando o catálogo informa o número. */
  contextLength: number | null
  /** Contexto já descrito pelo backend (ex.: `128k`). */
  contextWindow: string | null
  parameterSize: string | null
  isInstalled: boolean
  isRecommended: boolean
  /** Tamanho em disco, já formatado. */
  size: string | null
  /** Fornecedor no OpenRouter (`google/...` ➔ `google`). */
  vendor: string | null
}

function choice(base: Pick<AiModelChoice, 'id'> & Partial<AiModelChoice>): AiModelChoice {
  return {
    name: base.id,
    description: null,
    isFree: false,
    supportsTools: false,
    contextLength: null,
    contextWindow: null,
    parameterSize: null,
    isInstalled: false,
    isRecommended: false,
    size: null,
    vendor: null,
    ...base,
  }
}

export function openrouterChoices(models: readonly OpenRouterModelItem[]): AiModelChoice[] {
  return models.map((model) => {
    const parts = model.id.split('/')
    return choice({
      id: model.id,
      name: model.name || model.id,
      isFree: model.isFree,
      description: model.description ?? null,
      supportsTools: model.supportsTools ?? false,
      contextLength: model.contextLength != null ? Number(model.contextLength) : null,
      vendor: parts.length > 1 ? parts[0] : null,
    })
  })
}

export function opencodeChoices(models: readonly OpenCodeModelItem[]): AiModelChoice[] {
  return models.map((model) =>
    choice({
      id: model.id,
      name: model.name || model.id,
      isFree: model.isFree,
      description: model.description ?? null,
      // O gateway do OpenCode só lista modelos com function calling.
      supportsTools: model.supportsTools ?? true,
    })
  )
}

/** Recomendados do catálogo do backend, como opção de download. */
export function recommendedOllamaChoice(model: OllamaRecommendedModel): AiModelChoice {
  return choice({
    id: model.name,
    isFree: true,
    isInstalled: model.isInstalled,
    isRecommended: true,
    supportsTools: model.toolCallingOptimized,
    contextWindow: model.contextWindow,
    parameterSize: model.parameterSize,
    description: model.description,
  })
}

/** Instalados no Ollama mais os recomendados, sem repetir quem é as duas coisas. */
export function ollamaChoices(
  installed: readonly OllamaModelItem[],
  recommended: readonly OllamaRecommendedModel[]
): AiModelChoice[] {
  const byName = new Map<string, AiModelChoice>()
  for (const model of installed) {
    const lower = model.name.toLowerCase()
    byName.set(
      model.name,
      choice({
        id: model.name,
        isFree: true,
        isInstalled: true,
        size: model.size ? formatDecimalBytes(model.size) : null,
        parameterSize: model.parameterSize ?? null,
        description: `Instalado no Ollama (${model.parameterSize || 'tamanho não informado'})`,
        supportsTools: lower.includes('tool') || lower.includes('groq'),
      })
    )
  }
  for (const model of recommended) {
    const existing = byName.get(model.name)
    const suggestion = recommendedOllamaChoice(model)
    byName.set(
      model.name,
      existing
        ? {
            ...suggestion,
            isInstalled: true,
            size: existing.size,
            description: model.description || existing.description,
          }
        : suggestion
    )
  }
  return Array.from(byName.values())
}

/** Inclui na lista o modelo digitado à mão, para ele aparecer como escolhido. */
export function withTypedModel(
  choices: AiModelChoice[],
  typed: string | null | undefined,
  installed: boolean
): AiModelChoice[] {
  const id = (typed ?? '').trim()
  if (!id || choices.some((item) => item.id === id)) return choices
  return [...choices, choice({ id, isInstalled: installed, description: 'Modelo digitado' })]
}

/** Mesma regra do backend: `qwen3` casa `qwen3:latest` e `qwen3:8b`. */
export function isOllamaModelInstalled(
  installed: readonly OllamaModelItem[],
  name: string | null | undefined
): boolean {
  return isModelInstalled(
    installed.map((model) => model.name),
    name ?? ''
  )
}

export function findRecommendedOllamaModel(
  recommended: readonly OllamaRecommendedModel[],
  name: string | null | undefined
): OllamaRecommendedModel | null {
  const target = name ?? ''
  return recommended.find((model) => isModelInstalled([model.name], target)) ?? null
}

/** Busca por slug, nome, descrição, contexto, tamanho ou fornecedor. */
export function matchesModelQuery(item: AiModelChoice, query: string | null | undefined): boolean {
  const term = (query ?? '').trim().toLowerCase()
  if (!term) return true
  return [
    item.id,
    item.name,
    item.description,
    item.contextWindow,
    item.parameterSize,
    item.vendor,
  ].some((field) => (field ?? '').toLowerCase().includes(term))
}

/** `custom-filter` dos combobox de modelo: busca pelo item inteiro, não só pelo título. */
export const filterModelChoices: FilterFunction = (value, query, item) => {
  const raw = item?.raw as AiModelChoice | undefined
  if (!raw) return (value ?? '').toLowerCase().includes(query.trim().toLowerCase())
  return matchesModelQuery(raw, query)
}

/** Rótulo da janela de contexto: o do backend, ou o número em tokens. */
export function contextLabel(item: AiModelChoice): string | null {
  if (item.contextWindow) return item.contextWindow
  if (!item.contextLength) return null
  return `${formatCompactCount(item.contextLength)} tokens`
}
