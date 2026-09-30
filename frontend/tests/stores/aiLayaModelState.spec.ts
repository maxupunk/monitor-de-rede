import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { isLayaModelState, useAiLayaStore } from '@/stores/aiLaya'

describe('estado do modelo do Laya pelo SSE', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('carregando → na memória, sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const post = vi.spyOn(apiService, 'post')
    const store = useAiLayaStore()
    store.models = {
      online: true,
      errorMessage: null,
      installed: ['laya:multilingual'],
      loaded: [],
      options: [],
    }
    const base = { model: 'laya:multilingual', baseUrl: 'http://ollaya:11435' }

    store.applyModelState({ ...base, state: 'loading' })
    expect(store.inMemory('laya:multilingual')).toBe(false)

    store.applyModelState({ ...base, state: 'ready', loadMs: 8200 })
    expect(store.inMemory('laya:multilingual')).toBe(true)
    expect(store.modelState?.state).toBe('ready')

    expect(get).not.toHaveBeenCalled()
    expect(post).not.toHaveBeenCalled()
  })

  it('só aceita o payload do evento', () => {
    expect(isLayaModelState({ model: 'laya', baseUrl: 'x', state: 'loading' })).toBe(true)
    expect(isLayaModelState({ model: 'laya', baseUrl: 'x', state: 'outro' })).toBe(false)
    expect(isLayaModelState(null)).toBe(false)
  })
})
