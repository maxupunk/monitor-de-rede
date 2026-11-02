import { describe, expect, it } from 'vitest'
import {
  findOllamaChoice,
  ollamaChoices,
  withTypedModel,
} from '@/components/settings/ai/aiModelCatalog'
import {
  DEFAULT_OLLAMA_NUM_CTX,
  ollamaNumCtxFor,
  ollamaNumCtxOptions,
} from '@/components/settings/ai/aiProviders'
import type { OllamaRecommendedModel } from '@/stores/ai'
import { formatContextWindow } from '@/utils/formatters'

const recommended: OllamaRecommendedModel[] = [
  {
    name: 'lfm3-netmonitor:latest',
    description: 'LFM3 NetMonitor',
    parameterSize: '1.7B',
    contextWindow: '256k',
    contextLength: 262144,
    isInstalled: false,
    toolCallingOptimized: true,
    netmonitorTrained: true,
  },
  {
    name: 'phi4:14b',
    description: 'Phi-4',
    parameterSize: '14B',
    contextWindow: '16k',
    contextLength: 16384,
    isInstalled: false,
    toolCallingOptimized: false,
    netmonitorTrained: false,
  },
]

describe('janela de contexto do Ollama', () => {
  it('formata a janela em k binário, como o Ollama anuncia', () => {
    expect(formatContextWindow(262144)).toBe('256k')
    expect(formatContextWindow(40960)).toBe('40k')
    expect(formatContextWindow(16384, true)).toBe('16.384 tokens (16k)')
    expect(formatContextWindow(null)).toBe('—')
  })

  it('assume a janela do modelo e, sem ela, a recomendada', () => {
    expect(ollamaNumCtxFor(131072)).toBe(131072)
    expect(ollamaNumCtxFor(null)).toBe(DEFAULT_OLLAMA_NUM_CTX)
    expect(ollamaNumCtxFor(0)).toBe(DEFAULT_OLLAMA_NUM_CTX)
    expect(DEFAULT_OLLAMA_NUM_CTX).toBe(16384)
  })

  it('a janela do instalado (/api/show) vale mais que a da tabela', () => {
    const choices = ollamaChoices(
      [{ name: 'phi4:14b', size: 9_000_000_000, contextLength: 32768 }],
      recommended
    )
    expect(findOllamaChoice(choices, 'phi4:14b')?.contextLength).toBe(32768)
    expect(findOllamaChoice(choices, 'lfm3-netmonitor:latest')?.contextLength).toBe(262144)
  })

  it('leva o selo de treinado com o NetMonitor do backend para a escolha', () => {
    const choices = ollamaChoices(
      [{ name: 'lfm3-netmonitor:minha-tag', netmonitorTrained: true }, { name: 'qwen3:8b' }],
      recommended
    )
    expect(findOllamaChoice(choices, 'lfm3-netmonitor:latest')?.netmonitorTrained).toBe(true)
    expect(findOllamaChoice(choices, 'lfm3-netmonitor:minha-tag')?.netmonitorTrained).toBe(true)
    expect(findOllamaChoice(choices, 'qwen3:8b')?.netmonitorTrained).toBe(false)
    expect(findOllamaChoice(choices, 'phi4:14b')?.netmonitorTrained).toBe(false)
  })

  it('acha o modelo pelo nome curto e devolve nada para o digitado sem janela', () => {
    const choices = withTypedModel(
      ollamaChoices([{ name: 'qwen3:latest', contextLength: 40960 }], []),
      'meu-modelo',
      false
    )
    expect(findOllamaChoice(choices, 'qwen3')?.contextLength).toBe(40960)
    expect(findOllamaChoice(choices, 'meu-modelo')?.contextLength).toBeNull()
    expect(findOllamaChoice(choices, '')).toBeNull()
  })

  it('oferece só os degraus que cabem no modelo, mais a janela dele', () => {
    const options = ollamaNumCtxOptions(40960, 40960)
    expect(options.map((option) => option.value)).toEqual([8192, 16384, 32768, 40960])
    expect(options.at(-1)?.title).toBe('40.960 tokens (40k) — máximo do modelo')
    expect(options[1].title).toBe('16.384 tokens (16k) — recomendado')
  })

  it('mantém na lista o valor salvo, mesmo acima do máximo do modelo', () => {
    const options = ollamaNumCtxOptions(16384, 32768)
    expect(options.map((option) => option.value)).toEqual([8192, 16384, 32768])
    expect(options[1].title).toContain('recomendado · máximo do modelo')
    expect(options[2].title).toContain('acima do máximo do modelo')
  })

  it('sem janela conhecida, mostra todos os degraus', () => {
    expect(ollamaNumCtxOptions(null, 16384).map((option) => option.value)).toEqual([
      8192, 16384, 32768, 65536, 131072, 262144,
    ])
  })
})
