import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService, ApiError } from '@/services/apiService'
import type { AiDeciderSettings } from '@/bindings/AiDeciderSettings'
import type { DeciderModelOption } from '@/bindings/DeciderModelOption'
import type { DeciderProvider } from '@/bindings/DeciderProvider'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import type { DeciderBenchmarkReport } from '@/bindings/DeciderBenchmarkReport'
import type { DeciderBenchmarkView } from '@/bindings/DeciderBenchmarkView'
import type { DeciderDecisionsScoreView } from '@/bindings/DeciderDecisionsScoreView'
import type { DeciderModelSpecView } from '@/bindings/DeciderModelSpecView'
import type { DeciderModelsResponse } from '@/bindings/DeciderModelsResponse'
import type { ModelPullProgress } from '@/bindings/ModelPullProgress'
import type { TestDeciderResponse } from '@/bindings/TestDeciderResponse'
import { RECOMMENDED_MODEL } from '@/utils/decider'
import { streamModelPull } from '@/utils/modelPull'

export type { AiDeciderSettings, DeciderBenchmarkReport, DeciderModelOption, TestDeciderResponse }

/**
 * Estado do modelo no Ollama, pelo evento `decider:model_state`. Instalar
 * (o modelo que vem com o NetMonitor, na primeira vez) e carregar tiram o
 * modelo da rede e do disco: a tela mostra o andamento em vez de parecer
 * travada, e o tempo máximo por pergunta não conta isso.
 */
export interface DeciderModelState {
  model: string
  baseUrl: string
  state: DeciderModelPhase
  loadMs?: number
  /** `installing`: quanto já foi baixado, de 0 a 100. */
  percentage?: number
  message?: string
}

const MODEL_PHASES = ['installing', 'loading', 'ready', 'failed'] as const
export type DeciderModelPhase = (typeof MODEL_PHASES)[number]

export function isDeciderModelState(value: unknown): value is DeciderModelState {
  if (!value || typeof value !== 'object') return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.model === 'string' &&
    typeof candidate.baseUrl === 'string' &&
    MODEL_PHASES.includes(candidate.state as DeciderModelPhase)
  )
}

/** Padrões do backend (`services/ai/decider/config.rs`). */
export function defaultDeciderSettings(): AiDeciderSettings {
  return {
    enabled: false,
    provider: 'ollama',
    customProtocol: 'systemone',
    baseUrl: 'http://ollama:11434',
    model: RECOMMENDED_MODEL,
    minConfidence: 60,
    timeoutMs: 4000,
    features: {
      chatTools: true,
      deviceIdentity: true,
      interfaces: true,
      logEvents: false,
      incidentTriage: 'off',
      actionReview: true,
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

/** Decisor via Ollama: modelos disponíveis, testar o que ele decide e baixar o modelo. */
export const useAiDeciderStore = defineStore('aiDecider', () => {
  /** O que está gravado no servidor; o card compara com o formulário. */
  const saved = ref<AiDeciderSettings | null>(null)
  const saving = ref(false)
  const loadingModels = ref(false)
  const models = ref<DeciderModelsResponse | null>(null)

  const testing = ref(false)
  const testResult = ref<TestDeciderResponse | null>(null)

  const pulling = ref(false)
  const pullProgress = ref<ModelPullProgress | null>(null)
  /** De qual modelo e servidor é o download (e o erro) em `pullProgress`. */
  const pullTarget = ref<{ model: string; baseUrl: string } | null>(null)
  let pullController: AbortController | null = null

  async function load(): Promise<AiDeciderSettings> {
    saved.value = await apiService.get<AiDeciderSettings>('/ai/decider')
    return saved.value
  }

  /** Grava só o Decisor; devolve a mensagem de erro, ou `null` se deu certo. */
  async function save(decider: AiDeciderSettings): Promise<string | null> {
    saving.value = true
    try {
      saved.value = await apiService.put<AiDeciderSettings>('/ai/decider', decider)
      return null
    } catch (err) {
      return errorMessage(err, 'Falha ao salvar o Decisor')
    } finally {
      saving.value = false
    }
  }

  /** Os provedores de decisão que o backend oferece; lista fixa, lida uma vez. */
  const providers = ref<DeciderProviderView[]>([])

  async function loadProviders(): Promise<void> {
    if (providers.value.length) return
    try {
      providers.value = await apiService.get<DeciderProviderView[]>('/ai/decider/providers')
    } catch {
      providers.value = []
    }
  }

  function provider(id: DeciderProvider): DeciderProviderView | null {
    return providers.value.find((item) => item.id === id) ?? null
  }

  /**
   * Consulta pontual (ao abrir a tela, ao trocar provedor ou URL, depois de
   * um download), com o formulário inteiro: a lista depende do provedor, e
   * um Ollama em outra máquina pode pedir a chave ainda não gravada.
   */
  async function loadModels(decider: AiDeciderSettings): Promise<void> {
    loadingModels.value = true
    try {
      models.value = await apiService.post<DeciderModelsResponse>('/ai/decider/models', { decider })
    } catch (err) {
      models.value = {
        online: false,
        managesModels: decider.provider === 'ollama',
        errorMessage: errorMessage(err, 'Falha ao consultar o provedor'),
        installed: [],
        loaded: [],
        options: [],
      }
    } finally {
      loadingModels.value = false
    }
  }

  /** O modelo no catálogo devolvido pelo backend (com a ficha). */
  function option(model: string): DeciderModelOption | null {
    const target = model.trim().toLowerCase()
    return models.value?.options.find((item) => item.name.toLowerCase() === target) ?? null
  }

  /** A ficha do modelo; `null` para um fora do catálogo. */
  function spec(model: string): DeciderModelSpecView | null {
    return option(model)?.spec ?? null
  }

  /** O resultado no teste público; `null` para quem não foi avaliado. */
  function benchmark(model: string): DeciderBenchmarkView | null {
    return option(model)?.benchmark ?? null
  }

  /** A nota no benchmark das decisões do NetMonitor; `null` para quem não foi medido. */
  function decisions(model: string): DeciderDecisionsScoreView | null {
    return option(model)?.decisions ?? null
  }

  /** A página de benchmark: os modelos medidos nas decisões do NetMonitor. */
  const benchmarkReport = ref<DeciderBenchmarkReport | null>(null)
  const loadingBenchmark = ref(false)
  const benchmarkError = ref<string | null>(null)

  /** Consulta pontual ao abrir a página; o resultado não muda sem nova versão. */
  async function loadBenchmark(): Promise<void> {
    if (benchmarkReport.value || loadingBenchmark.value) return
    loadingBenchmark.value = true
    benchmarkError.value = null
    try {
      benchmarkReport.value = await apiService.get<DeciderBenchmarkReport>('/ai/decider/benchmark')
    } catch (err) {
      benchmarkError.value = errorMessage(err, 'Falha ao carregar o benchmark dos modelos')
    } finally {
      loadingBenchmark.value = false
    }
  }

  /**
   * Para o do catálogo vale o veredito do backend, que conhece os apelidos
   * (`kev` baixado é o `kev:4b`); o resto, pela regra de nomes.
   */
  function installed(model: string): boolean {
    return option(model)?.installed ?? isModelInstalled(models.value?.installed ?? [], model)
  }

  /** O último estado de carregamento recebido pelo SSE. */
  const modelState = ref<DeciderModelState | null>(null)

  /** O modelo está na memória do Ollama agora? */
  function inMemory(model: string): boolean {
    return isModelInstalled(models.value?.loaded ?? [], model)
  }

  function applyModelState(state: DeciderModelState): void {
    modelState.value = state
    if (state.state === 'ready' && models.value && !inMemory(state.model)) {
      models.value = { ...models.value, loaded: [...models.value.loaded, state.model] }
    }
  }

  /** Carrega o modelo em segundo plano; o progresso chega pelo SSE. */
  async function warmUp(decider: AiDeciderSettings): Promise<void> {
    modelState.value = { model: decider.model, baseUrl: decider.baseUrl, state: 'loading' }
    try {
      await apiService.post('/ai/decider/load', { decider })
    } catch (err) {
      modelState.value = {
        model: decider.model,
        baseUrl: decider.baseUrl,
        state: 'failed',
        message: errorMessage(err, 'Falha ao carregar o modelo'),
      }
    }
  }

  async function test(decider: AiDeciderSettings, question?: string): Promise<TestDeciderResponse> {
    testing.value = true
    try {
      testResult.value = await apiService.post<TestDeciderResponse>('/ai/decider/test', {
        decider,
        question: question?.trim() || null,
      })
    } catch (err) {
      testResult.value = {
        success: false,
        online: false,
        modelInstalled: false,
        latencyMs: 0,
        message: errorMessage(err, 'Falha ao testar o Decisor'),
        model: null,
        groups: [],
      }
    } finally {
      testing.value = false
    }
    return testResult.value
  }

  /** Baixa o modelo e, dando certo, testa de novo para a tela refletir. */
  async function pull(decider: AiDeciderSettings, question?: string): Promise<boolean> {
    if (pulling.value) return false
    pulling.value = true
    pullTarget.value = { model: decider.model, baseUrl: decider.baseUrl }
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
      await streamModelPull('/ai/decider/pull', { decider }, pullController.signal, (progress) => {
        pullProgress.value = progress
      })
      pullProgress.value = null
      await loadModels(decider)
      await test(decider, question)
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
    providers,
    loadProviders,
    provider,
    loadingModels,
    models,
    testing,
    testResult,
    pulling,
    pullProgress,
    pullTarget,
    loadModels,
    installed,
    spec,
    benchmark,
    decisions,
    benchmarkReport,
    loadingBenchmark,
    benchmarkError,
    loadBenchmark,
    modelState,
    inMemory,
    applyModelState,
    warmUp,
    test,
    pull,
    cancelPull,
  }
})
