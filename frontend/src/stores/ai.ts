import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import { apiService, ApiError } from '@/services/apiService'
import type { OpenCodeModelItem } from '@/bindings/OpenCodeModelItem'
import type { OpenCodeModelsResponse } from '@/bindings/OpenCodeModelsResponse'
import type { OpenRouterModelItem } from '@/bindings/OpenRouterModelItem'
import type { OpenRouterModelsResponse } from '@/bindings/OpenRouterModelsResponse'
import type { AiChart } from '@/bindings/AiChart'
import type { AiResponseStyle } from '@/bindings/AiResponseStyle'

export type {
  AiChart,
  AiResponseStyle,
  OpenCodeModelItem,
  OpenCodeModelsResponse,
  OpenRouterModelItem,
  OpenRouterModelsResponse,
}

export interface AiSettings {
  enabled: boolean
  activeDriver: 'opencode' | 'openrouter' | 'ollama' | string
  opencodeBaseUrl?: string | null
  opencodeApiKey?: string | null
  opencodeModel?: string | null
  openrouterApiKey?: string | null
  openrouterModel?: string | null
  ollamaBaseUrl?: string | null
  ollamaModel?: string | null
  allowActiveTools: boolean
  requireToolConfirmation: boolean
  responseStyle: AiResponseStyle
  customSystemPrompt?: string | null
}

export interface TestConnectionResponse {
  success: boolean
  latencyMs: number
  message: string
  model?: string | null
}

export interface OllamaModelItem {
  name: string
  size?: number | null
  parameterSize?: string | null
  modifiedAt?: string | null
}

export interface OllamaRecommendedModel {
  name: string
  description: string
  parameterSize: string
  isInstalled: boolean
  toolCallingOptimized: boolean
}

export interface OllamaModelsResponse {
  installed: OllamaModelItem[]
  recommended: OllamaRecommendedModel[]
  isOnline: boolean
  errorMessage?: string | null
}

export interface OllamaPullProgress {
  status: string
  digest?: string | null
  total?: number | null
  completed?: number | null
  percentage?: number | null
  done: boolean
  error?: string | null
}

export interface AiToolCallState {
  id: string
  name: string
  arguments: Record<string, unknown>
  result?: Record<string, unknown> | null
  /** Gráfico desenhado no chat; a IA recebe só o resumo em `result`. */
  chart?: AiChart | null
  status: 'running' | 'done' | 'error'
}

export interface AiDisplayMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  toolCalls?: AiToolCallState[]
  isStreaming?: boolean
  error?: string | null
}

export const useAiStore = defineStore('ai', () => {
  const settings = ref<AiSettings | null>(null)
  const loadingSettings = ref(false)
  const savingSettings = ref(false)
  const testingConnection = ref(false)
  const testResult = ref<TestConnectionResponse | null>(null)

  const installedOllamaModels = ref<OllamaModelItem[]>([])
  const recommendedOllamaModels = ref<OllamaRecommendedModel[]>([])
  const loadingOllamaModels = ref(false)
  const ollamaOnline = ref(true)
  const ollamaError = ref<string | null>(null)
  const pullingModelName = ref<string | null>(null)
  const pullProgress = ref<OllamaPullProgress | null>(null)
  const pullError = ref<string | null>(null)
  let activePullAbortController: AbortController | null = null

  const opencodeModels = ref<OpenCodeModelItem[]>([])
  const loadingOpencodeModels = ref(false)
  const opencodeOnline = ref(true)
  const opencodeError = ref<string | null>(null)

  const openrouterModels = ref<OpenRouterModelItem[]>([])
  const loadingOpenrouterModels = ref(false)
  const openrouterOnline = ref(true)
  const openrouterError = ref<string | null>(null)

  const saveError = ref<string | null>(null)

  const messages = ref<AiDisplayMessage[]>([])
  const isStreaming = ref(false)
  const isDrawerOpen = ref(false)
  const streamError = ref<string | null>(null)

  let activeAbortController: AbortController | null = null

  async function loadSettings() {
    loadingSettings.value = true
    try {
      settings.value = await apiService.get<AiSettings>('/ai/settings')
    } catch (err) {
      console.error('Falha ao carregar configurações de IA:', err)
    } finally {
      loadingSettings.value = false
    }
  }

  async function saveSettings(
    newSettings: AiSettings
  ): Promise<{ success: boolean; message?: string }> {
    saveError.value = null
    savingSettings.value = true
    try {
      settings.value = await apiService.put<AiSettings>('/ai/settings', newSettings)
      return { success: true }
    } catch (err: unknown) {
      const msg =
        err instanceof ApiError
          ? err.message
          : err instanceof Error
            ? err.message
            : 'Falha ao salvar configurações de IA'
      saveError.value = msg
      console.error('Falha ao salvar configurações de IA:', err)
      return { success: false, message: msg }
    } finally {
      savingSettings.value = false
    }
  }

  async function testConnection(input: {
    driver: string
    baseUrl?: string | null
    apiKey?: string | null
    model: string
  }): Promise<TestConnectionResponse> {
    testingConnection.value = true
    testResult.value = null
    try {
      const res = await apiService.post<TestConnectionResponse>('/ai/test-connection', input)
      testResult.value = res
      return res
    } catch (err) {
      const msg = err instanceof ApiError ? err.message : 'Falha ao conectar com o provedor de IA'
      const failRes: TestConnectionResponse = {
        success: false,
        latencyMs: 0,
        message: msg,
      }
      testResult.value = failRes
      return failRes
    } finally {
      testingConnection.value = false
    }
  }

  async function sendMessage(content: string, deviceId?: number | null) {
    if (!content.trim() || isStreaming.value) return

    streamError.value = null
    const userMsgId = `user-${Date.now()}`
    const assistantMsgId = `asst-${Date.now()}`

    messages.value.push({
      id: userMsgId,
      role: 'user',
      content: content.trim(),
    })

    const assistantMsg = reactive<AiDisplayMessage>({
      id: assistantMsgId,
      role: 'assistant',
      content: '',
      toolCalls: [],
      isStreaming: true,
    })
    messages.value.push(assistantMsg)

    isStreaming.value = true
    const controller = new AbortController()
    activeAbortController = controller

    try {
      // Monta histórico de mensagens para a API
      const apiMessages = messages.value
        .filter((m) => m.id !== assistantMsgId && !m.error)
        .map((m) => ({
          role: m.role,
          content: m.content,
        }))

      const response = await apiService.postStream(
        '/ai/chat/stream',
        {
          messages: apiMessages,
          deviceId: deviceId || undefined,
        },
        controller.signal
      )

      const reader = response.body?.getReader()
      if (!reader) {
        throw new Error('Não foi possível inicializar o leitor de stream')
      }

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() || ''

        for (const line of lines) {
          const trimmed = line.trim()
          if (!trimmed || trimmed.startsWith(':')) continue

          const prefix = trimmed.startsWith('data: ')
            ? 'data: '
            : trimmed.startsWith('data:')
              ? 'data:'
              : null
          if (prefix) {
            const jsonStr = trimmed.slice(prefix.length).trim()
            try {
              const event = JSON.parse(jsonStr)

              if (event.type === 'textDelta' && typeof event.content === 'string') {
                assistantMsg.content += event.content
              } else if (event.type === 'toolCall') {
                if (!assistantMsg.toolCalls) assistantMsg.toolCalls = []
                assistantMsg.toolCalls.push({
                  id: event.id,
                  name: event.name,
                  arguments: event.arguments || {},
                  status: 'running',
                })
              } else if (event.type === 'toolResult') {
                const targetTool = assistantMsg.toolCalls?.find((t) => t.id === event.id)
                if (targetTool) {
                  targetTool.result = event.result
                  targetTool.chart = event.chart ?? null
                  targetTool.status = event.result?.error ? 'error' : 'done'
                }
              } else if (event.type === 'error') {
                assistantMsg.error = event.message
                streamError.value = event.message
              } else if (event.type === 'done') {
                assistantMsg.isStreaming = false
              }
            } catch {
              // Ignora linhas parciais
            }
          }
        }
      }
    } catch (err: unknown) {
      if (err instanceof Error && err.name === 'AbortError') {
        assistantMsg.content += ' [Geração interrompida]'
      } else {
        const msg = err instanceof Error ? err.message : 'Erro no processamento da resposta'
        assistantMsg.error = msg
        streamError.value = msg
      }
    } finally {
      assistantMsg.isStreaming = false
      isStreaming.value = false
      activeAbortController = null
    }
  }

  function cancelGeneration() {
    if (activeAbortController) {
      activeAbortController.abort()
      activeAbortController = null
    }
    isStreaming.value = false
  }

  function clearMessages() {
    cancelGeneration()
    messages.value = []
    streamError.value = null
  }

  function toggleDrawer() {
    isDrawerOpen.value = !isDrawerOpen.value
  }

  async function loadOpencodeModels(apiKey?: string | null, baseUrl?: string | null) {
    loadingOpencodeModels.value = true
    try {
      const params = new URLSearchParams()
      if (apiKey && apiKey !== '********') params.append('api_key', apiKey)
      if (baseUrl) params.append('base_url', baseUrl)
      const qs = params.toString()
      const url = qs ? `/ai/opencode/models?${qs}` : '/ai/opencode/models'

      const data = await apiService.get<OpenCodeModelsResponse>(url)
      opencodeModels.value = data.models || []
      opencodeOnline.value = data.isOnline
      opencodeError.value = data.errorMessage || null
    } catch (err) {
      console.error('Falha ao listar modelos do OpenCode:', err)
      opencodeOnline.value = false
      opencodeError.value =
        err instanceof ApiError
          ? err.message
          : err instanceof Error
            ? err.message
            : 'Falha ao consultar modelos do OpenCode'
    } finally {
      loadingOpencodeModels.value = false
    }
  }

  async function loadOpenrouterModels(apiKey?: string | null) {
    loadingOpenrouterModels.value = true
    try {
      const params = new URLSearchParams()
      if (apiKey && apiKey !== '********') params.append('api_key', apiKey)
      const qs = params.toString()
      const url = qs ? `/ai/openrouter/models?${qs}` : '/ai/openrouter/models'

      const data = await apiService.get<OpenRouterModelsResponse>(url)
      openrouterModels.value = data.models || []
      openrouterOnline.value = data.isOnline
      openrouterError.value = data.errorMessage || null
    } catch (err) {
      console.error('Falha ao listar modelos do OpenRouter:', err)
      openrouterOnline.value = false
      openrouterError.value =
        err instanceof ApiError
          ? err.message
          : err instanceof Error
            ? err.message
            : 'Falha ao consultar modelos do OpenRouter'
    } finally {
      loadingOpenrouterModels.value = false
    }
  }

  async function loadOllamaModels(baseUrl?: string | null) {
    loadingOllamaModels.value = true
    try {
      const url = baseUrl
        ? `/ai/ollama/models?base_url=${encodeURIComponent(baseUrl)}`
        : '/ai/ollama/models'
      const data = await apiService.get<OllamaModelsResponse>(url)
      installedOllamaModels.value = data.installed || []
      recommendedOllamaModels.value = data.recommended || []
      ollamaOnline.value = data.isOnline
      ollamaError.value = data.errorMessage || null
    } catch (err) {
      console.error('Falha ao listar modelos do Ollama:', err)
      ollamaOnline.value = false
      ollamaError.value =
        err instanceof ApiError
          ? err.message
          : err instanceof Error
            ? err.message
            : 'Falha ao conectar com o Ollama'
    } finally {
      loadingOllamaModels.value = false
    }
  }

  async function pullOllamaModel(
    modelName: string,
    baseUrl?: string | null,
    onSuccess?: () => void
  ): Promise<boolean> {
    const trimmed = modelName.trim()
    if (!trimmed || pullingModelName.value) return false

    pullError.value = null
    pullingModelName.value = trimmed
    pullProgress.value = {
      status: 'Iniciando download...',
      percentage: 0,
      done: false,
    }

    const controller = new AbortController()
    activePullAbortController = controller

    try {
      const response = await apiService.postStream(
        '/ai/ollama/pull',
        {
          model: trimmed,
          baseUrl: baseUrl || undefined,
        },
        controller.signal
      )

      const reader = response.body?.getReader()
      if (!reader) {
        throw new Error('Não foi possível inicializar leitura do download')
      }

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() || ''

        for (const line of lines) {
          const trimmedLine = line.trim()
          if (!trimmedLine || trimmedLine.startsWith(':')) continue

          if (trimmedLine.startsWith('data: ')) {
            const jsonStr = trimmedLine.slice(6).trim()
            try {
              const event = JSON.parse(jsonStr) as OllamaPullProgress
              pullProgress.value = event

              if (event.error) {
                throw new Error(event.error)
              }

              if (event.done) {
                await loadOllamaModels(baseUrl)
                if (onSuccess) onSuccess()
                return true
              }
            } catch (jsonErr) {
              if (jsonErr instanceof Error && jsonErr.message !== 'Unexpected end of JSON input') {
                throw jsonErr
              }
            }
          }
        }
      }

      await loadOllamaModels(baseUrl)
      if (onSuccess) onSuccess()
      return true
    } catch (err: unknown) {
      if (err instanceof Error && err.name === 'AbortError') {
        pullProgress.value = {
          status: 'Download cancelado pelo usuário',
          done: true,
          error: 'Cancelado',
        }
      } else {
        const msg =
          err instanceof ApiError
            ? err.message
            : err instanceof Error
              ? err.message
              : 'Falha durante o download do modelo'
        pullError.value = msg
        pullProgress.value = {
          status: 'Erro no download',
          done: true,
          error: msg,
        }
      }
      return false
    } finally {
      pullingModelName.value = null
      activePullAbortController = null
    }
  }

  function cancelOllamaPull() {
    if (activePullAbortController) {
      activePullAbortController.abort()
      activePullAbortController = null
    }
    pullingModelName.value = null
  }

  return {
    settings,
    loadingSettings,
    savingSettings,
    testingConnection,
    testResult,
    installedOllamaModels,
    recommendedOllamaModels,
    loadingOllamaModels,
    ollamaOnline,
    ollamaError,
    pullingModelName,
    pullProgress,
    pullError,
    messages,
    isStreaming,
    isDrawerOpen,
    streamError,
    loadSettings,
    saveSettings,
    testConnection,
    opencodeModels,
    loadingOpencodeModels,
    opencodeOnline,
    opencodeError,
    openrouterModels,
    loadingOpenrouterModels,
    openrouterOnline,
    openrouterError,
    saveError,
    loadOpencodeModels,
    loadOpenrouterModels,
    loadOllamaModels,
    pullOllamaModel,
    cancelOllamaPull,
    sendMessage,
    cancelGeneration,
    clearMessages,
    toggleDrawer,
  }
})
