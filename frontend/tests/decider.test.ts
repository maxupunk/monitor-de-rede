import { describe, expect, it } from 'vitest'
import type { DeciderModelSpecView } from '@/bindings/DeciderModelSpecView'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import { defaultDeciderSettings } from '@/stores/aiDecider'
import {
  formForProvider,
  isLocalOllaya,
  memoryLabel,
  decisionsFact,
  modelFacts,
  RECOMMENDED_MODEL,
  providerMissing,
  timeoutForModel,
  usesCustomUrl,
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
  family: 'Decima Network',
  language: 'multilingual',
  downloadMb: 510,
  suggestedTimeoutMs: 2000,
  recommended: true,
  bundled: true,
}

const kev08: DeciderModelSpecView = {
  family: 'Kev',
  language: 'english',
  downloadMb: 1800,
  suggestedTimeoutMs: 10000,
  recommended: false,
  bundled: false,
}

/** Sem tamanho de download conhecido. */
const semDownload: DeciderModelSpecView = {
  family: 'Outro',
  language: 'english',
  suggestedTimeoutMs: 4000,
  recommended: false,
  bundled: false,
}

describe('ficha do modelo', () => {
  const network = { overall: 0.9321, p95Ms: 980.6 }

  it('só números do benchmark do NetMonitor: nota, p95, língua e download', () => {
    expect(modelFacts(decider, network).map((fact) => fact.label)).toEqual([
      'NetMonitor 93% · 981 ms (p95, CPU)',
      'Multilíngue (100+ línguas)',
      '510 MB de download',
    ])
  })

  it('o Kev avisa a língua', () => {
    const facts = modelFacts(kev08, { overall: 0.6102, p95Ms: 11483.4 })
    expect(facts.map((fact) => fact.label)).toEqual([
      'NetMonitor 61% · 11483 ms (p95, CPU)',
      'Sem português',
      '1.8 GB de download',
    ])
    expect(facts[1]?.color).toBe('warning')
  })

  it('fora do catálogo e sem medição, sem chip', () => {
    expect(modelFacts(null, null)).toEqual([])
  })

  it('o download é o piso da memória', () => {
    expect(memoryLabel(kev08)).toBe('pelo menos 1.8 GB')
    expect(memoryLabel(semDownload)).toBe('alguns GB')
  })
})

describe('tempo máximo ao trocar de modelo', () => {
  it('valor não mexido segue o sugerido do novo modelo, nos dois sentidos', () => {
    expect(timeoutForModel(2000, decider, kev08, 4000)).toBe(10000)
    expect(timeoutForModel(10000, kev08, decider, 4000)).toBe(2000)
  })

  it('valor ajustado à mão só sobe', () => {
    expect(timeoutForModel(3000, decider, kev08, 4000)).toBe(10000)
    expect(timeoutForModel(5000, semDownload, decider, 4000)).toBe(5000)
  })

  it('modelo fora do catálogo não mexe no tempo', () => {
    expect(timeoutForModel(2000, decider, null, 4000)).toBe(2000)
    expect(timeoutForModel(4000, null, kev08, 4000)).toBe(10000)
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
    const next = formForProvider(form, decider, cloudflare, 1500)
    expect(next.provider).toBe('cloudflare')
    expect(next.baseUrl).toBe('https://api.cloudflare.com/client/v4')
    expect(next.model).toBe('clef-flash')
    expect(next.apiKey).toBeUndefined()
    expect(next.accountId).toBeUndefined()
    expect(next.timeoutMs).toBe(3000)
  })

  it('URL de um provedor nunca vai para outro (Ollaya local não vira OpenRouter)', () => {
    const local = { ...form, baseUrl: 'http://127.0.0.1:11435' }
    expect(formForProvider(local, decider, cloudflare, 1500).baseUrl).toBe(
      'https://api.cloudflare.com/client/v4'
    )
  })

  it('volta a URL já digitada para o provedor nesta tela', () => {
    const outra = { ...form, provider: 'cloudflare' as const, baseUrl: cloudflare.defaultBaseUrl }
    expect(formForProvider(outra, cloudflare, ollaya, 1500, 'http://10.0.0.5:11435').baseUrl).toBe(
      'http://10.0.0.5:11435'
    )
  })

  it('URL fora do padrão só quando difere do padrão do provedor', () => {
    const cf = { ...form, provider: 'cloudflare' as const }
    expect(
      usesCustomUrl({ ...cf, baseUrl: 'https://api.cloudflare.com/client/v4/' }, cloudflare)
    ).toBe(false)
    expect(usesCustomUrl({ ...cf, baseUrl: '' }, cloudflare)).toBe(false)
    expect(usesCustomUrl({ ...cf, baseUrl: 'http://127.0.0.1:11435' }, cloudflare)).toBe(true)
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

describe('benchmark do NetMonitor', () => {
  const network = { overall: 0.919, p95Ms: 664 }

  it('vira o primeiro chip da ficha, com a cor da régua', () => {
    const facts = modelFacts(null, network)
    expect(facts[0]?.label).toBe('NetMonitor 92% · 664 ms (p95, CPU)')
    expect(decisionsFact(network).color).toBe('success')
    expect(decisionsFact(network).short).toBe('92%')
    expect(decisionsFact({ overall: 0.61, p95Ms: 11483 }).color).toBe('info')
    expect(decisionsFact({ overall: 0.37, p95Ms: 300 }).color).toBe('error')
  })

  it('o padrão é o Decima Network, que vem com o NetMonitor', () => {
    expect(defaultDeciderSettings().model).toBe(RECOMMENDED_MODEL)
    expect(RECOMMENDED_MODEL).toBe('netmonitor/decima-network:v11')
    expect(defaultDeciderSettings().timeoutMs).toBe(4000)
  })
})
