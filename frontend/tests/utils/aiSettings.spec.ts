import { describe, expect, it } from 'vitest'
import { formatHourOfDay } from '@/utils/formatters'
import { activeToolsFlags, activeToolsMode } from '@/components/settings/ai/aiAutomationModes'
import {
  DEFAULT_MODELS,
  OPENCODE_BASE_URL,
  connectionTestInput,
  defaultAiSettings,
  extractModelId,
  modelForDriver,
  normalizeAiSettings,
  resolveDriver,
} from '@/components/settings/ai/aiProviders'
import {
  contextLabel,
  findRecommendedOllamaModel,
  isOllamaModelInstalled,
  matchesModelQuery,
  ollamaChoices,
  openrouterChoices,
  withTypedModel,
} from '@/components/settings/ai/aiModelCatalog'
import type { OllamaRecommendedModel } from '@/stores/ai'

const recommended: OllamaRecommendedModel = {
  name: 'ornith-1.5:9b',
  description: 'Recomendado',
  parameterSize: '9B',
  contextWindow: '128k',
  isInstalled: false,
  toolCallingOptimized: true,
}

describe('formatHourOfDay', () => {
  it('formata a hora cheia com dois dígitos', () => {
    expect(formatHourOfDay(0)).toBe('00:00')
    expect(formatHourOfDay(8)).toBe('08:00')
    expect(formatHourOfDay(23)).toBe('23:00')
  })
})

describe('modo dos testes ativos', () => {
  it('lê os dois booleanos do contrato como uma escolha só', () => {
    expect(activeToolsMode(false, true)).toBe('off')
    expect(activeToolsMode(false, false)).toBe('off')
    expect(activeToolsMode(true, true)).toBe('confirm')
    expect(activeToolsMode(true, false)).toBe('auto')
  })

  it('grava o modo de volta nos dois booleanos', () => {
    expect(activeToolsFlags('confirm', false)).toEqual({
      allowActiveTools: true,
      requireToolConfirmation: true,
    })
    expect(activeToolsFlags('auto', true)).toEqual({
      allowActiveTools: true,
      requireToolConfirmation: false,
    })
  })

  it('desligar preserva a preferência de confirmação', () => {
    expect(activeToolsFlags('off', true)).toEqual({
      allowActiveTools: false,
      requireToolConfirmation: true,
    })
  })
})

describe('aiProviders', () => {
  it('extrai o slug de texto ou de item de combobox', () => {
    expect(extractModelId('  qwen3:8b ')).toBe('qwen3:8b')
    expect(extractModelId({ id: 'a/b' })).toBe('a/b')
    expect(extractModelId({ name: 'mistral' })).toBe('mistral')
    expect(extractModelId(null)).toBe('')
  })

  it('driver desconhecido cai no Ollama', () => {
    expect(resolveDriver('openrouter')).toBe('openrouter')
    expect(resolveDriver('outro')).toBe('ollama')
  })

  it('normaliza endpoint, modelo retirado, contexto e limite de resumos', () => {
    const settings = {
      ...defaultAiSettings(),
      opencodeBaseUrl: '',
      openrouterModel: 'meta-llama/llama-3.3-70b-instruct:free',
      ollamaNumCtx: null,
      customSystemPrompt: 'mantido',
    }
    settings.proactive = { ...settings.proactive, maxSummariesPerHour: '' as unknown as number }

    const normalized = normalizeAiSettings(settings)

    expect(normalized.opencodeBaseUrl).toBe(OPENCODE_BASE_URL)
    expect(normalized.openrouterModel).toBe(DEFAULT_MODELS.openrouter)
    expect(normalized.ollamaNumCtx).toBe(16384)
    expect(normalized.proactive.maxSummariesPerHour).toBe(6)
    expect(normalized.customSystemPrompt).toBe('mantido')
    // Cópia: a entrada não muda.
    expect(normalized.proactive).not.toBe(settings.proactive)
    expect(settings.opencodeBaseUrl).toBe('')
  })

  it('monta o teste de conexão do provedor ativo com os padrões', () => {
    const base = defaultAiSettings()
    expect(connectionTestInput({ ...base, activeDriver: 'opencode', opencodeModel: '' })).toEqual({
      driver: 'opencode',
      baseUrl: OPENCODE_BASE_URL,
      apiKey: null,
      model: DEFAULT_MODELS.opencode,
    })
    expect(
      connectionTestInput({ ...base, activeDriver: 'openrouter', openrouterApiKey: 'sk-or' })
    ).toEqual({
      driver: 'openrouter',
      baseUrl: null,
      apiKey: 'sk-or',
      model: DEFAULT_MODELS.openrouter,
    })
  })

  it('lê o modelo do provedor ativo', () => {
    expect(
      modelForDriver({ ...defaultAiSettings(), activeDriver: 'ollama', ollamaModel: 'x' })
    ).toBe('x')
  })
})

describe('aiModelCatalog', () => {
  it('casa tags do Ollama pela mesma regra do backend', () => {
    const installed = [{ name: 'qwen3:latest' }, { name: 'mistral:7b' }]
    expect(isOllamaModelInstalled(installed, 'qwen3')).toBe(true)
    expect(isOllamaModelInstalled(installed, 'mistral')).toBe(true)
    expect(isOllamaModelInstalled(installed, 'llama3')).toBe(false)
    expect(findRecommendedOllamaModel([recommended], 'ornith-1.5:9b')?.name).toBe('ornith-1.5:9b')
  })

  it('une instalados e recomendados sem repetir', () => {
    const choices = ollamaChoices(
      [{ name: 'ornith-1.5:9b', size: 5_000_000_000 }, { name: 'llama3:8b' }],
      [recommended]
    )
    expect(choices).toHaveLength(2)
    const ornith = choices.find((item) => item.id === 'ornith-1.5:9b')
    expect(ornith).toMatchObject({ isInstalled: true, isRecommended: true, size: '5 GB' })
  })

  it('inclui o modelo digitado à mão uma vez só', () => {
    const choices = ollamaChoices([], [recommended])
    expect(withTypedModel(choices, 'meu-modelo', false)).toHaveLength(2)
    expect(withTypedModel(choices, 'ornith-1.5:9b', false)).toHaveLength(1)
  })

  it('descreve o contexto e busca pelo item inteiro', () => {
    const [model] = openrouterChoices([
      { id: 'google/gemma', name: 'Gemma', isFree: true, contextLength: BigInt(131072) },
    ])
    expect(model.vendor).toBe('google')
    expect(contextLabel(model)).toBe('131,1 mil tokens')
    expect(matchesModelQuery(model, 'GOOGLE')).toBe(true)
    expect(matchesModelQuery(model, 'claude')).toBe(false)
  })
})
