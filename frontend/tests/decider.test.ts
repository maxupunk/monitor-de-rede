import { describe, expect, it } from 'vitest'
import type { DeciderModelSpecView } from '@/bindings/DeciderModelSpecView'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import { defaultDeciderSettings } from '@/stores/aiDecider'
import {
  formForProvider,
  isLocalOllaya,
  memoryLabel,
  benchmarkFact,
  isNearChance,
  modelFacts,
  providerMissing,
  timeoutForModel,
} from '@/utils/decider'

describe('isLocalOllaya', () => {
  it('reconhece o Ollaya desta máquina', () => {
    expect(isLocalOllaya('http://ollaya:11435')).toBe(true)
    expect(isLocalOllaya('http://localhost:11435/')).toBe(true)
    expect(isLocalOllaya('127.0.0.1:11435')).toBe(true)
    expect(isLocalOllaya('http://[::1]:11435')).toBe(true)
    expect(isLocalOllaya('http://host.docker.internal:11435')).toBe(true)
    expect(isLocalOllaya('')).toBe(true)
  })

  it('outra máquina não é local', () => {
    expect(isLocalOllaya('http://192.168.1.50:11435')).toBe(false)
    expect(isLocalOllaya('https://decider.exemplo.com.br')).toBe(false)
    expect(isLocalOllaya('http://ollaya-gpu:11435')).toBe(false)
  })
})

const decider: DeciderModelSpecView = {
  family: 'Decisor',
  language: 'multilingual',
  memoryMb: 4000,
  latencyMs: 307,
  gpuRecommended: false,
  accuracy: 0.361,
  suggestedTimeoutMs: 1500,
  recommended: true,
}

const kev4b: DeciderModelSpecView = {
  family: 'Kev',
  language: 'english',
  downloadMb: 9500,
  memoryMb: 11000,
  latencyMs: 354,
  gpuRecommended: true,
  accuracy: 0.669,
  suggestedTimeoutMs: 2500,
  recommended: false,
}

/** Publicado sem memória nem acerto (como o decision:eos). */
const semNumeros: DeciderModelSpecView = {
  family: 'Decision',
  language: 'english',
  downloadMb: 1500,
  latencyMs: 2000,
  gpuRecommended: false,
  suggestedTimeoutMs: 6000,
  recommended: false,
}

describe('ficha do modelo', () => {
  it('o Decisor é multilíngue e roda em CPU', () => {
    const labels = modelFacts(decider).map((fact) => fact.label)
    expect(labels).toContain('Multilíngue (100+ línguas)')
    expect(labels).toContain('Roda bem em CPU')
    expect(labels).toContain('307 ms por 5 perguntas (CPU)')
    expect(labels.some((label) => label.includes('download'))).toBe(false)
  })

  it('o Kev avisa a língua, mostra acerto, velocidade, tamanho e GPU', () => {
    const facts = modelFacts(kev4b)
    expect(facts.map((fact) => fact.label)).toEqual([
      'Sem português',
      'Acerto 67% (teste do autor)',
      '354 ms por 5 perguntas (RTX 4090)',
      '9.5 GB de download',
      '~11 GB de memória',
      'GPU recomendada',
    ])
    expect(facts.filter((fact) => fact.color === 'warning')).toHaveLength(2)
  })

  it('sem número publicado, sem chip — e o download vira o piso da memória', () => {
    const labels = modelFacts(semNumeros).map((fact) => fact.label)
    expect(labels.some((label) => label.startsWith('Acerto'))).toBe(false)
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

const ollaya: DeciderProviderView = {
  id: 'ollaya',
  label: 'Ollaya',
  description: '',
  protocol: 'ollaya',
  defaultBaseUrl: 'http://ollaya:11435',
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
    const next = formForProvider(form, ollaya, decider, cloudflare, 1500)
    expect(next.provider).toBe('cloudflare')
    expect(next.baseUrl).toBe('https://api.cloudflare.com/client/v4')
    expect(next.model).toBe('clef-flash')
    expect(next.apiKey).toBeUndefined()
    expect(next.accountId).toBeUndefined()
    expect(next.timeoutMs).toBe(3000)
  })

  it('URL mudada pelo usuário fica (Ollaya em outra máquina, gateway próprio)', () => {
    const proprio = { ...form, baseUrl: 'http://10.0.0.5:11435' }
    expect(formForProvider(proprio, ollaya, decider, cloudflare, 1500).baseUrl).toBe(
      'http://10.0.0.5:11435'
    )
  })

  it('diz o que falta, em português', () => {
    const vazio = {
      ...defaultDeciderSettings(),
      provider: 'cloudflare' as const,
      model: 'clef-flash',
    }
    expect(providerMissing(vazio, cloudflare)).toBe('Faltam o ID da conta e a chave de acesso')
    expect(providerMissing({ ...vazio, accountId: 'c', apiKey: '********' }, cloudflare)).toBeNull()
    expect(providerMissing(defaultDeciderSettings(), ollaya)).toBeNull()
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
    expect(isNearChance(laya)).toBe(true)
    expect(isNearChance(kev08)).toBe(false)
    expect(isNearChance(null)).toBe(false)
  })

  it('o padrão novo é o Kev 0.8B com o tempo dele', () => {
    expect(defaultDeciderSettings().model).toBe('kev:0.8b')
    expect(defaultDeciderSettings().timeoutMs).toBe(4000)
  })
})
