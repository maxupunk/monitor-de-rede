import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService, ApiError } from '@/services/apiService'
import type { AiLayaSettings } from '@/bindings/AiLayaSettings'
import type { LayaModelOption } from '@/bindings/LayaModelOption'
import type { LayaModelsResponse } from '@/bindings/LayaModelsResponse'
import type { ModelPullProgress } from '@/bindings/ModelPullProgress'
import type { TestLayaResponse } from '@/bindings/TestLayaResponse'
import { streamModelPull } from '@/utils/modelPull'

export type { AiLayaSettings, LayaModelOption, TestLayaResponse }

/**
 * Estado do modelo na memória do Ollaya, pelo evento `laya:model_state`.
 * Carregar tira o modelo do disco (5–20 s): a tela mostra "carregando" em vez
 * de parecer travada, e o tempo máximo por pergunta não conta isso.
 */
export interface LayaModelState {
  model: string
  baseUrl: string
  state: 'loading' | 'ready' | 'failed'
  loadMs?: number
  message?: string
}

export function isLayaModelState(value: unknown): value is LayaModelState {
  if (!value || typeof value !== 'object') return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.model === 'string' &&
    typeof candidate.baseUrl === 'string' &&
    (candidate.state === 'loading' || candidate.state === 'ready' || candidate.state === 'failed')
  )
}

/** Padrões do backend (`services/ai/laya/config.rs`). */
export function defaultLayaSettings(): AiLayaSettings {
  return {
    enabled: false,
    baseUrl: 'http://ollaya:11435',
    model: 'laya',
    minConfidence: 60,
    timeoutMs: 1500,
    features: {
      chatTools: true,
      deviceIdentity: true,
      interfaces: true,
      logEvents: false,
      incidentTriage: 'off',
    },
  }
}

/**
 * O modelo pedido está entre os instalados? Mesma regra do backend
 * (`local_models::is_installed`): `laya` casa `laya:latest` e `laya:en`.
 */
export function isModelInstalled(installed: readonly string[], wanted: string): boolean {
  const target = wanted.trim().toLowerCase()
  if (!target) return false
  const bare = target.endsWith(':latest') ? target.slice(0, -':latest'.length) : target
  return installed.some((name) => {
    const current = name.toLowerCase()
    return current === target || current === bare || current.startsWith(`${bare}:`)
  })
}

function errorMessage(err: unknown, fallback: string): string {
  if (err instanceof ApiError || err instanceof Error) return err.message
  return fallback
}

/** Laya via Ollaya: modelos disponíveis, testar o que ele decide e baixar o modelo. */
export const useAiLayaStore = defineStore('aiLaya', () => {
  /** O que está gravado no servidor; o card compara com o formulário. */
  const saved = ref<AiLayaSettings | null>(null)
  const saving = ref(false)
  const loadingModels = ref(false)
  const models = ref<LayaModelsResponse | null>(null)

  const testing = ref(false)
  const testResult = ref<TestLayaResponse | null>(null)

  const pulling = ref(false)
  const pullProgress = ref<ModelPullProgress | null>(null)
  let pullController: AbortController | null = null

  async function load(): Promise<AiLayaSettings> {
    saved.value = await apiService.get<AiLayaSettings>('/ai/laya')
    return saved.value
  }

  /** Grava só o Laya; devolve a mensagem de erro, ou `null` se deu certo. */
  async function save(laya: AiLayaSettings): Promise<string | null> {
    saving.value = true
    try {
      saved.value = await apiService.put<AiLayaSettings>('/ai/laya', laya)
      return null
    } catch (err) {
      return errorMessage(err, 'Falha ao salvar o Laya')
    } finally {
      saving.value = false
    }
  }

  /** Consulta pontual: ao abrir a tela, ao trocar a URL, depois de um download. */
  async function loadModels(baseUrl: string): Promise<void> {
    loadingModels.value = true
    try {
      const query = baseUrl.trim() ? `?base_url=${encodeURIComponent(baseUrl.trim())}` : ''
      models.value = await apiService.get<LayaModelsResponse>(`/ai/laya/models${query}`)
    } catch (err) {
      models.value = {
        online: false,
        errorMessage: errorMessage(err, 'Falha ao consultar o Ollaya'),
        installed: [],
        loaded: [],
        options: [],
      }
    } finally {
      loadingModels.value = false
    }
  }

  function installed(model: string): boolean {
    return isModelInstalled(models.value?.installed ?? [], model)
  }

  /** O último estado de carregamento recebido pelo SSE. */
  const modelState = ref<LayaModelState | null>(null)

  /** O modelo está na memória do Ollaya agora? */
  function inMemory(model: string): boolean {
    return isModelInstalled(models.value?.loaded ?? [], model)
  }

  function applyModelState(state: LayaModelState): void {
    modelState.value = state
    if (state.state === 'ready' && models.value && !inMemory(state.model)) {
      models.value = { ...models.value, loaded: [...models.value.loaded, state.model] }
    }
  }

  /** Carrega o modelo em segundo plano; o progresso chega pelo SSE. */
  async function warmUp(laya: AiLayaSettings): Promise<void> {
    modelState.value = { model: laya.model, baseUrl: laya.baseUrl, state: 'loading' }
    try {
      await apiService.post('/ai/laya/load', { laya })
    } catch (err) {
      modelState.value = {
        model: laya.model,
        baseUrl: laya.baseUrl,
        state: 'failed',
        message: errorMessage(err, 'Falha ao carregar o modelo'),
      }
    }
  }

  async function test(laya: AiLayaSettings, question?: string): Promise<TestLayaResponse> {
    testing.value = true
    try {
      testResult.value = await apiService.post<TestLayaResponse>('/ai/laya/test', {
        laya,
        question: question?.trim() || null,
      })
    } catch (err) {
      testResult.value = {
        success: false,
        online: false,
        modelInstalled: false,
        latencyMs: 0,
        message: errorMessage(err, 'Falha ao testar o Laya'),
        model: null,
        groups: [],
      }
    } finally {
      testing.value = false
    }
    return testResult.value
  }

  /** Baixa o modelo e, dando certo, testa de novo para a tela refletir. */
  async function pull(laya: AiLayaSettings, question?: string): Promise<boolean> {
    if (pulling.value) return false
    pulling.value = true
    pullProgress.value = {
      status: 'Iniciando download...',
      digest: null,
      total: null,
      completed: null,
      percentage: 0,
      done: false,
      error: null,
    }
    pullController = new AbortController()
    try {
      await streamModelPull('/ai/laya/pull', { laya }, pullController.signal, (progress) => {
        pullProgress.value = progress
      })
      pullProgress.value = null
      await loadModels(laya.baseUrl)
      await test(laya, question)
      return true
    } catch (err) {
      const cancelled = err instanceof Error && err.name === 'AbortError'
      pullProgress.value = {
        status: cancelled ? 'Download cancelado' : 'Erro no download',
        digest: null,
        total: null,
        completed: null,
        percentage: null,
        done: true,
        error: cancelled ? 'Cancelado' : errorMessage(err, 'Falha durante o download do modelo'),
      }
      return false
    } finally {
      pulling.value = false
      pullController = null
    }
  }

  function cancelPull() {
    pullController?.abort()
  }

  return {
    saved,
    saving,
    load,
    save,
    loadingModels,
    models,
    testing,
    testResult,
    pulling,
    pullProgress,
    loadModels,
    installed,
    modelState,
    inMemory,
    applyModelState,
    warmUp,
    test,
    pull,
    cancelPull,
  }
})
