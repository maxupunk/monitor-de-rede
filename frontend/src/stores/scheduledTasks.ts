/**
 * Tarefas agendadas: toda a conversa HTTP da tela (`/api/scheduled-tasks`).
 *
 * Tempo real só pelo SSE: `scheduled_task:updated` relê a lista se ela já
 * foi aberta e avisa o histórico aberto (`lastEvent`). Nada aqui consulta a
 * API sozinho.
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { SchedulableTool } from '@/bindings/SchedulableTool'
import type { ScheduledTask } from '@/bindings/ScheduledTask'
import type { ScheduledTaskEvent } from '@/bindings/ScheduledTaskEvent'
import type { ScheduledTaskInput } from '@/bindings/ScheduledTaskInput'
import type { ScheduledTaskPreview } from '@/bindings/ScheduledTaskPreview'
import type { ScheduledTaskPreviewInput } from '@/bindings/ScheduledTaskPreviewInput'
import type { ScheduledTaskStarted } from '@/bindings/ScheduledTaskStarted'

const BASE = '/scheduled-tasks'

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

export function isScheduledTaskEvent(data: unknown): data is ScheduledTaskEvent {
  if (data === null || typeof data !== 'object') return false
  const event = data as Record<string, unknown>
  return typeof event.id === 'number'
}

export const useScheduledTasksStore = defineStore('scheduledTasks', () => {
  const tasks = ref<ScheduledTask[]>([])
  const loaded = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const catalog = ref<SchedulableTool[]>([])
  const catalogLoading = ref(false)
  const catalogError = ref<string | null>(null)

  /** A última notícia do SSE — o histórico aberto relê quando é da tarefa dele. */
  const lastEvent = ref<ScheduledTaskEvent | null>(null)

  /** `quiet`: releitura pelo SSE, sem piscar o carregamento da tabela. */
  async function fetchTasks(options: { quiet?: boolean } = {}): Promise<void> {
    if (!options.quiet) loading.value = true
    error.value = null
    try {
      tasks.value = await apiService.get<ScheduledTask[]>(BASE)
      loaded.value = true
    } catch (err: unknown) {
      error.value = describe(err, 'Não foi possível carregar as tarefas agendadas.')
    } finally {
      loading.value = false
    }
  }

  /** O catálogo não muda com o uso: carrega uma vez. */
  async function fetchCatalog(): Promise<void> {
    if (catalog.value.length > 0 || catalogLoading.value) return
    catalogLoading.value = true
    catalogError.value = null
    try {
      catalog.value = await apiService.get<SchedulableTool[]>(`${BASE}/catalog`)
    } catch (err: unknown) {
      catalogError.value = describe(err, 'Não foi possível carregar o que pode ser agendado.')
    } finally {
      catalogLoading.value = false
    }
  }

  function toolOf(name: string): SchedulableTool | undefined {
    return catalog.value.find((tool) => tool.name === name)
  }

  function replace(saved: ScheduledTask) {
    const index = tasks.value.findIndex((task) => task.id === saved.id)
    if (index >= 0) tasks.value.splice(index, 1, saved)
    else tasks.value = [...tasks.value, saved].sort((a, b) => a.name.localeCompare(b.name))
  }

  /** Conferência do formulário: erros, frase da agenda, próximas execuções, risco. */
  function preview(input: ScheduledTaskPreviewInput): Promise<ScheduledTaskPreview> {
    return apiService.post<ScheduledTaskPreview>(`${BASE}/preview`, input)
  }

  async function create(input: ScheduledTaskInput): Promise<ScheduledTask> {
    const saved = await apiService.post<ScheduledTask>(BASE, input)
    replace(saved)
    return saved
  }

  async function update(id: number, input: ScheduledTaskInput): Promise<ScheduledTask> {
    const saved = await apiService.put<ScheduledTask>(`${BASE}/${id}`, input)
    replace(saved)
    return saved
  }

  async function setEnabled(id: number, enabled: boolean): Promise<ScheduledTask> {
    const saved = await apiService.put<ScheduledTask>(`${BASE}/${id}/enabled`, { enabled })
    replace(saved)
    return saved
  }

  async function remove(id: number): Promise<void> {
    await apiService.delete(`${BASE}/${id}`)
    tasks.value = tasks.value.filter((task) => task.id !== id)
  }

  /** "Rodar agora": responde na hora; o desfecho chega pelo SSE. */
  async function runNow(id: number): Promise<number> {
    const started = await apiService.post<ScheduledTaskStarted>(`${BASE}/${id}/run`, {})
    return started.runId
  }

  /** `scheduled_task:updated` — só relê se a lista estiver aberta. */
  function applyUpdated(data: unknown) {
    if (!isScheduledTaskEvent(data)) return
    lastEvent.value = data
    if (loaded.value) void fetchTasks({ quiet: true })
  }

  return {
    tasks,
    loaded,
    loading,
    error,
    catalog,
    catalogLoading,
    catalogError,
    lastEvent,
    fetchTasks,
    fetchCatalog,
    toolOf,
    preview,
    create,
    update,
    setEnabled,
    remove,
    runNow,
    applyUpdated,
  }
})
