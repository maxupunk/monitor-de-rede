import { describe, expect, it } from 'vitest'
import type { LayaModelSpecView } from '@/bindings/LayaModelSpecView'
import { isLocalOllaya, memoryLabel, modelFacts, timeoutForModel } from '@/utils/ollaya'

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
    expect(isLocalOllaya('https://laya.exemplo.com.br')).toBe(false)
    expect(isLocalOllaya('http://ollaya-gpu:11435')).toBe(false)
  })
})

const laya: LayaModelSpecView = {
  family: 'Laya',
  language: 'multilingual',
  memoryMb: 4000,
  latencyMs: 307,
  gpuRecommended: false,
  accuracy: 0.361,
  suggestedTimeoutMs: 1500,
  recommended: true,
}

const kev4b: LayaModelSpecView = {
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
const semNumeros: LayaModelSpecView = {
  family: 'Decision',
  language: 'english',
  downloadMb: 1500,
  latencyMs: 2000,
  gpuRecommended: false,
  suggestedTimeoutMs: 6000,
  recommended: false,
}

describe('ficha do modelo', () => {
  it('o Laya é multilíngue e roda em CPU', () => {
    const labels = modelFacts(laya).map((fact) => fact.label)
    expect(labels).toContain('Multilíngue (100+ línguas)')
    expect(labels).toContain('Roda bem em CPU')
    expect(labels).toContain('307 ms por 5 perguntas (CPU)')
    expect(labels.some((label) => label.includes('download'))).toBe(false)
  })

  it('o Kev avisa a língua, mostra acerto, velocidade, tamanho e GPU', () => {
    const facts = modelFacts(kev4b)
    expect(facts.map((fact) => fact.label)).toEqual([
      'Sem português',
      'Acerto 67%',
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
    expect(timeoutForModel(1500, laya, kev4b, 1500)).toBe(2500)
    expect(timeoutForModel(2500, kev4b, laya, 1500)).toBe(1500)
  })

  it('valor ajustado à mão só sobe', () => {
    expect(timeoutForModel(2000, laya, kev4b, 1500)).toBe(2500)
    expect(timeoutForModel(8000, kev4b, laya, 1500)).toBe(8000)
  })

  it('modelo fora do catálogo não mexe no tempo', () => {
    expect(timeoutForModel(1500, laya, null, 1500)).toBe(1500)
    expect(timeoutForModel(1500, null, kev4b, 1500)).toBe(2500)
  })
})
