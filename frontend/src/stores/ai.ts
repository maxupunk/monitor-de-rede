import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import { apiService, ApiError } from '@/services/apiService'
import type { OpenCodeModelItem } from '@/bindings/OpenCodeModelItem'
import type { OpenCodeModelsResponse } from '@/bindings/OpenCodeModelsResponse'
import type { OpenRouterModelItem } from '@/bindings/OpenRouterModelItem'
import type { OpenRouterModelsResponse } from '@/bindings/OpenRouterModelsResponse'
import type { AiChart } from '@/bindings/AiChart'
import type { AiDigest } from '@/bindings/AiDigest'
import type { AiProactiveSettings } from '@/bindings/AiProactiveSettings'
import type { AiResponseStyle } from '@/bindings/AiResponseStyle'
import type { ExecuteToolResponse } from '@/bindings/ExecuteToolResponse'
import {
  applyChatEvent,
  toApiMessages,
  type AiDisplayMessage,
  type AiToolCallState,
} from '@/utils/aiChatStream'
import { useAiConversationsStore } from './aiConversations'
import { readSseJson } from '@/utils/sseReader'

export type {
  AiChart,
  AiDigest,
  AiDisplayMessage,
  AiProactiveSettings,
  AiResponseStyle,
  AiToolCallState,
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
  allowActions: boolean
  responseStyle: AiResponseStyle
  proactive: AiProactiveSettings
  customSystemPrompt?: string | null
}

/** Tela de onde o chat foi aberto: vira contexto no prompt da IA. */
export interface AiChatContext {
  deviceId?: number | null
  monitorId?: number | null
  alertId?: number | null
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

  const conversations = useAiConversationsStore()

  const latestDigest = ref<AiDigest | null>(null)
  const loadingDigest = ref(false)
  const runningDigest = ref(false)
  const digestError = ref<string | null>(null)

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

  function errorMessage(err: unknown, fallback: string): string {
    if (err instanceof ApiError) return err.message
    if (err instanceof Error) return err.message
    return fallback
  }

  async function sendMessage(content: string, context: AiChatContext = {}) {
    if (!content.trim() || isStreaming.value) return

    streamError.value = null
    const stamp = Date.now()
    messages.value.push({ id: `user-${stamp}`, role: 'user', content: content.trim() })
    const history = toApiMessages(messages.value)

    const assistantMsg = reactive<AiDisplayMessage>({
      id: `asst-${stamp}`,
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
      const response = await apiService.postStream(
        '/ai/chat/stream',
        {
          messages: history,
          deviceId: context.deviceId ?? undefined,
          monitorId: context.monitorId ?? undefined,
          alertId: context.alertId ?? undefined,
        },
        controller.signal
      )
      const reader = response.body?.getReader()
      if (!reader) {
        throw new Error('Não foi possível inicializar o leitor de stream')
      }
      await readSseJson(reader, (event) => applyChatEvent(assistantMsg, event))
      if (assistantMsg.error) streamError.value = assistantMsg.error
    } catch (err: unknown) {
      if (err instanceof Error && err.name === 'AbortError') {
        assistantMsg.content += ' [Geração interrompida]'
      } else {
        const msg = errorMessage(err, 'Erro no processamento da resposta')
        assistantMsg.error = msg
        streamError.value = msg
      }
    } finally {
      assistantMsg.isStreaming = false
      isStreaming.value = false
      activeAbortController = null
      void conversations.persist(messages.value)
    }
  }

  function cancelGeneration() {
    if (activeAbortController) {
      activeAbortController.abort()
      activeAbortController = null
    }
    isStreaming.value = false
  }

  /** Começa uma conversa nova; a atual continua salva no histórico. */
  function newConversation() {
    cancelGeneration()
    messages.value = []
    streamError.value = null
    conversations.startNew()
  }

  /** Mantido para as telas que já chamavam "limpar": abre uma conversa nova. */
  function clearMessages() {
    newConversation()
  }

  /** Reabre uma conversa salva. Ferramenta que ficou rodando vira falha. */
  async function openConversation(id: number) {
    cancelGeneration()
    const loaded = await conversations.open<AiDisplayMessage>(id)
    if (!loaded) return
    streamError.value = null
    messages.value = loaded.map((message) => ({
      ...message,
      isStreaming: false,
      toolCalls: message.toolCalls?.map((tool) =>
        tool.status === 'running'
          ? { ...tool, status: 'error', result: { error: 'Interrompido' } }
          : tool
      ),
    }))
  }

  async function deleteConversation(id: number) {
    if (conversations.activeId === id) newConversation()
    await conversations.remove(id)
  }

  /**
   * "Diagnosticar com IA" das telas de alerta, dispositivo e monitor: abre o
   * painel numa conversa nova já com a pergunta e o contexto da tela.
   */
  function askAbout(prompt: string, context: AiChatContext): boolean {
    if (isStreaming.value) return false
    newConversation()
    isDrawerOpen.value = true
    void sendMessage(prompt, context)
    return true
  }

  function findTool(messageId: string, toolId: string): AiToolCallState | null {
    const message = messages.value.find((item) => item.id === messageId)
    return message?.toolCalls?.find((tool) => tool.id === toolId) ?? null
  }

  /** Executa a ação que a IA propôs e o usuário confirmou. */
  async function confirmTool(messageId: string, toolId: string) {
    const tool = findTool(messageId, toolId)
    if (!tool || tool.status !== 'awaiting') return
    tool.status = 'running'
    try {
      const response = await apiService.post<ExecuteToolResponse>('/ai/tools/execute', {
        name: tool.name,
        arguments: tool.arguments,
      })
      tool.result = response.result
      tool.chart = response.chart ?? null
      tool.status = response.result?.error ? 'error' : 'done'
    } catch (err: unknown) {
      tool.result = { error: errorMessage(err, 'Falha ao executar a ação') }
      tool.status = 'error'
    } finally {
      void conversations.persist(messages.value)
    }
  }

  function cancelTool(messageId: string, toolId: string) {
    const tool = findTool(messageId, toolId)
    if (!tool || tool.status !== 'awaiting') return
    tool.status = 'cancelled'
    void conversations.persist(messages.value)
  }

  async function loadLatestDigest() {
    loadingDigest.value = true
    digestError.value = null
    try {
      latestDigest.value = await apiService.get<AiDigest | null>('/ai/digest/latest')
    } catch (err: unknown) {
      digestError.value = errorMessage(err, 'Falha ao carregar o resumo da rede')
    } finally {
      loadingDigest.value = false
    }
  }

  /** Gera o resumo da rede agora (ação explícita do usuário). */
  async function runDigest() {
    runningDigest.value = true
    digestError.value = null
    try {
      latestDigest.value = await apiService.post<AiDigest>('/ai/digest/run', {})
    } catch (err: unknown) {
      digestError.value = errorMessage(err, 'Falha ao gerar o resumo da rede')
    } finally {
      runningDigest.value = false
    }
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
    newConversation,
    openConversation,
    deleteConversation,
    askAbout,
    confirmTool,
    cancelTool,
    latestDigest,
    loadingDigest,
    runningDigest,
    digestError,
    loadLatestDigest,
    runDigest,
    toggleDrawer,
  }
})
