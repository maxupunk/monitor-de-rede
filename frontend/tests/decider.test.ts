import { describe, expect, it } from 'vitest'
import type { DeciderModelSpecView } from '@/bindings/DeciderModelSpecView'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import { defaultDeciderSettings } from '@/stores/aiDecider'
import {
  formForProvider,
  isLocalOllama,
  memoryLabel,
  benchmarkFact,
  decisionsFact,
  isNearChance,
  modelFacts,
  primaryScore,
  RECOMMENDED_MODEL,
  providerMissing,
  timeoutForModel,
  usesCustomUrl,
} from '@/utils/decider'

describe('isLocalOllama', () => {
  it('reconhece o Ollama desta máquina', () => {
    expect(isLocalOllama('http://ollama:11434')).toBe(true)
    expect(isLocalOllama('http://localhost:11434/')).toBe(true)
    expect(isLocalOllama('127.0.0.1:11434')).toBe(true)
    expect(isLocalOllama('http://[::1]:11434')).toBe(true)
    expect(isLocalOllama('http://host.docker.internal:11434')).toBe(true)
    expect(isLocalOllama('')).toBe(true)
  })

  it('outra máquina não é local', () => {
    expect(isLocalOllama('http://192.168.1.50:11434')).toBe(false)
    expect(isLocalOllama('https://decider.exemplo.com.br')).toBe(false)
    expect(isLocalOllama('http://ollama-gpu:11434')).toBe(false)
  })
})

const decider: DeciderModelSpecView = {
  family: 'Laya Network',
  language: 'multilingual',
  memoryMb: 4000,
  latencyMs: 307,
  gpuRecommended: false,
  suggestedTimeoutMs: 1500,
  recommended: true,
  bundled: false,
}

const kev4b: DeciderModelSpecView = {
  family: 'Kev',
  language: 'english',
  downloadMb: 9500,
  memoryMb: 11000,
  gpuRecommended: true,
  suggestedTimeoutMs: 2500,
  recommended: false,
  bundled: false,
}

/** Sem memória medida. */
const semNumeros: DeciderModelSpecView = {
  family: 'Decision',
  language: 'english',
  downloadMb: 1500,
  latencyMs: 2000,
  gpuRecommended: false,
  suggestedTimeoutMs: 6000,
  recommended: false,
  bundled: false,
}

describe('ficha do modelo', () => {
  it('o Laya Network é multilíngue e roda em CPU', () => {
    const labels = modelFacts(decider).map((fact) => fact.label)
    expect(labels).toContain('Multilíngue (100+ línguas)')
    expect(labels).toContain('Roda bem em CPU')
    expect(labels).toContain('307 ms por decisão em CPU (mediana)')
    expect(labels.some((label) => label.includes('download'))).toBe(false)
  })

  it('o Kev avisa a língua, mostra tamanho e GPU — sem tempo em CPU, sem chip de tempo', () => {
    const facts = modelFacts(kev4b)
    expect(facts.map((fact) => fact.label)).toEqual([
      'Sem português',
      '9.5 GB de download',
      '~11 GB de memória',
      'GPU recomendada',
    ])
    expect(facts.filter((fact) => fact.color === 'warning')).toHaveLength(2)
  })

  it('sem número medido, sem chip — e o download vira o piso da memória', () => {
    const labels = modelFacts(semNumeros).map((fact) => fact.label)
    expect(labels.some((label) => label.includes('memória'))).toBe(false)
    expect(memoryLabel(kev4b)).toBe('aproximadamente 11 GB')
    expect(memoryLabel(semNumeros)).toBe('pelo menos 1.5 GB')
  })
})

describe('tempo máximo ao trocar de modelo', () => {
  it('valor não mexido segue o sugerido do novo modelo, nos dois sentidos', () => {
    expect(timeoutForModel(1500, decider, kev4b, 1500)).toBe(2500)
    expect(timeoutForModel(2500, kev4b, decider, 1500)).toBe(1500)
  })

  it('valor ajustado à mão só sobe', () => {
    expect(timeoutForModel(2000, decider, kev4b, 1500)).toBe(2500)
    expect(timeoutForModel(8000, kev4b, decider, 1500)).toBe(8000)
  })

  it('modelo fora do catálogo não mexe no tempo', () => {
    expect(timeoutForModel(1500, decider, null, 1500)).toBe(1500)
    expect(timeoutForModel(1500, null, kev4b, 1500)).toBe(2500)
  })
})

const ollama: DeciderProviderView = {
  id: 'ollama',
  label: 'Ollama',
  description: '',
  protocol: 'ollama',
  defaultBaseUrl: 'http://ollama:11434',
  defaultModel: 'laya',
  models: [],
  requiresKey: false,
  requiresAccount: false,
  cloud: false,
  managesModels: true,
  suggestedTimeoutMs: 1500,
}

const cloudflare: DeciderProviderView = {
  id: 'cloudflare',
  label: 'Cloudflare (Clef)',
  description: '',
  protocol: 'cloudflare',
  defaultBaseUrl: 'https://api.cloudflare.com/client/v4',
  defaultModel: 'clef-flash',
  models: [],
  requiresKey: true,
  requiresAccount: true,
  keyUrl: 'https://dash.cloudflare.com/profile/api-tokens',
  cloud: true,
  managesModels: false,
  suggestedTimeoutMs: 3000,
}

describe('troca de provedor', () => {
  // Formulário da época do Laya: tempo de 1,5 s, o sugerido para ele.
  const form = { ...defaultDeciderSettings(), timeoutMs: 1500, apiKey: 'segredo', accountId: 'x' }

  it('leva os padrões do novo e nunca a chave nem a conta do anterior', () => {
    const next = formForProvider(form, decider, cloudflare, 1500)
    expect(next.provider).toBe('cloudflare')
    expect(next.baseUrl).toBe('https://api.cloudflare.com/client/v4')
    expect(next.model).toBe('clef-flash')
    expect(next.apiKey).toBeUndefined()
    expect(next.accountId).toBeUndefined()
    expect(next.timeoutMs).toBe(3000)
  })

  it('URL de um provedor nunca vai para outro (Ollama local não vira OpenRouter)', () => {
    const local = { ...form, baseUrl: 'http://127.0.0.1:11434' }
    expect(formForProvider(local, decider, cloudflare, 1500).baseUrl).toBe(
      'https://api.cloudflare.com/client/v4'
    )
  })

  it('volta a URL já digitada para o provedor nesta tela', () => {
    const outra = { ...form, provider: 'cloudflare' as const, baseUrl: cloudflare.defaultBaseUrl }
    expect(formForProvider(outra, cloudflare, ollama, 1500, 'http://10.0.0.5:11434').baseUrl).toBe(
      'http://10.0.0.5:11434'
    )
  })

  it('URL fora do padrão só quando difere do padrão do provedor', () => {
    const cf = { ...form, provider: 'cloudflare' as const }
    expect(
      usesCustomUrl({ ...cf, baseUrl: 'https://api.cloudflare.com/client/v4/' }, cloudflare)
    ).toBe(false)
    expect(usesCustomUrl({ ...cf, baseUrl: '' }, cloudflare)).toBe(false)
    expect(usesCustomUrl({ ...cf, baseUrl: 'http://127.0.0.1:11434' }, cloudflare)).toBe(true)
  })

  it('diz o que falta, em português', () => {
    const vazio = {
      ...defaultDeciderSettings(),
      provider: 'cloudflare' as const,
      model: 'clef-flash',
    }
    expect(providerMissing(vazio, cloudflare)).toBe('Faltam o ID da conta e a chave de acesso')
    expect(providerMissing({ ...vazio, accountId: 'c', apiKey: '********' }, cloudflare)).toBeNull()
    expect(providerMissing(defaultDeciderSettings(), ollama)).toBeNull()
  })
})

describe('teste público', () => {
  const kev08 = { accuracy: 0.631, portuguese: 0.619, latencyMs: 86 }
  const laya = { accuracy: 0.369, portuguese: 0.381, latencyMs: 19 }
  const jeb4 = { accuracy: 0.905, portuguese: 0.893, latencyMs: 205 }

  it('vale antes da língua declarada e do acerto do autor', () => {
    const labels = modelFacts(null, kev08).map((fact) => fact.label)
    expect(labels).toEqual(['Acerto 63% · PT 62%'])
  })

  it('cor pela régua: bom, útil, perto do chute', () => {
    expect(benchmarkFact(jeb4).color).toBe('success')
    expect(benchmarkFact(kev08).color).toBe('info')
    expect(benchmarkFact(laya).color).toBe('error')
    expect(benchmarkFact(kev08).short).toBe('63%')
  })

  it('perto do chute só abaixo de 60%', () => {
    expect(isNearChance(primaryScore(laya, null))).toBe(true)
    expect(isNearChance(primaryScore(kev08, null))).toBe(false)
    expect(isNearChance(null)).toBe(false)
  })
})

describe('benchmark do NetMonitor', () => {
  const network = { overall: 0.919, p95Ms: 664 }
  const kev08 = { accuracy: 0.631, portuguese: 0.619, latencyMs: 86 }

  it('a nota das decisões do NetMonitor vale antes do teste público', () => {
    expect(primaryScore(kev08, { overall: 0.693, p95Ms: 11052 })).toBe(0.693)
    expect(primaryScore(kev08, null)).toBe(0.631)
    expect(primaryScore(null, null)).toBeNull()
  })

  it('vira o primeiro chip da ficha, com a cor da régua', () => {
    const facts = modelFacts(null, null, network)
    expect(facts[0]?.label).toBe('NetMonitor 92% · 664 ms (p95, CPU)')
    expect(decisionsFact(network).color).toBe('success')
    expect(decisionsFact(network).short).toBe('92%')
  })

  it('o padrão é o Laya Network, que vem com o NetMonitor', () => {
    expect(defaultDeciderSettings().model).toBe(RECOMMENDED_MODEL)
    expect(RECOMMENDED_MODEL).toBe('netmonitor/laya-network:v2')
    expect(defaultDeciderSettings().timeoutMs).toBe(4000)
  })
})
