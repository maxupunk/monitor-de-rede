import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { isDeciderModelState, useAiDeciderStore } from '@/stores/aiDecider'

describe('estado do modelo do Decisor pelo SSE', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('carregando → na memória, sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const post = vi.spyOn(apiService, 'post')
    const store = useAiDeciderStore()
    store.models = {
      online: true,
      errorMessage: null,
      installed: ['laya:multilingual'],
      loaded: [],
      options: [],
    }
    const base = { model: 'laya:multilingual', baseUrl: 'http://ollama:11434' }

    store.applyModelState({ ...base, state: 'loading' })
    expect(store.inMemory('laya:multilingual')).toBe(false)

    store.applyModelState({ ...base, state: 'ready', loadMs: 8200 })
    expect(store.inMemory('laya:multilingual')).toBe(true)
    expect(store.modelState?.state).toBe('ready')

    expect(get).not.toHaveBeenCalled()
    expect(post).not.toHaveBeenCalled()
  })

  it('só aceita o payload do evento', () => {
    expect(isDeciderModelState({ model: 'laya', baseUrl: 'x', state: 'loading' })).toBe(true)
    expect(isDeciderModelState({ model: 'laya', baseUrl: 'x', state: 'outro' })).toBe(false)
    expect(isDeciderModelState(null)).toBe(false)
  })
  it('modelo do catálogo vale o veredito do backend, que conhece os apelidos', () => {
    const store = useAiDeciderStore()
    store.models = {
      online: true,
      errorMessage: null,
      installed: ['kev:latest', 'kev:9b'],
      loaded: [],
      options: [
        { name: 'kev:4b', description: 'Kev 4B', installed: true },
        { name: 'kev:0.8b', description: 'Kev 0.8B', installed: false },
      ],
    }
    expect(store.installed('kev:4b')).toBe(true)
    expect(store.installed('kev:0.8b')).toBe(false)
    expect(store.installed('kev:9b')).toBe(true)
    expect(store.spec('kev:4b')).toBeNull()
  })
})
