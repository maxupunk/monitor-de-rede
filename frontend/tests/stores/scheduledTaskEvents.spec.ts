import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useEventsStore } from '@/stores/events'
import { useScheduledTasksStore } from '@/stores/scheduledTasks'

class FakeEventSource {
  static latest: FakeEventSource | null = null
  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null

  constructor(_url: string) {
    FakeEventSource.latest = this
  }

  close() {}
}

function emit(type: string, data: unknown) {
  FakeEventSource.latest?.onmessage?.({
    data: JSON.stringify({ type, timestamp: '2026-10-23T12:00:00Z', data }),
  } as MessageEvent<string>)
}

describe('SSE das tarefas agendadas', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.stubGlobal('EventSource', FakeEventSource)
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('com a lista fechada, só guarda a notícia — sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    useEventsStore().connect()
    emit('scheduled_task:updated', { id: 7, name: 'Ping', status: 'running' })
    expect(useScheduledTasksStore().lastEvent).toEqual({ id: 7, name: 'Ping', status: 'running' })
    expect(get).not.toHaveBeenCalled()
  })

  it('com a lista aberta, relê sem piscar o carregamento', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue([])
    const store = useScheduledTasksStore()
    await store.fetchTasks()
    useEventsStore().connect()
    emit('scheduled_task:updated', { id: 7, name: 'Ping', status: 'success' })
    expect(store.loading).toBe(false)
    expect(get).toHaveBeenCalledTimes(2)
    expect(get).toHaveBeenLastCalledWith('/scheduled-tasks')
  })

  it('no feed, só a falha e o "não rodou" viram notícia', () => {
    vi.spyOn(apiService, 'get').mockResolvedValue([])
    const events = useEventsStore()
    events.connect()
    emit('scheduled_task:updated', { id: 1, name: 'A', status: 'running' })
    emit('scheduled_task:updated', { id: 1, name: 'A', status: 'success' })
    emit('scheduled_task:updated', { id: 2, name: 'B', status: 'failed' })
    emit('scheduled_task:updated', { id: 3, name: 'C', status: 'skipped' })
    emit('scheduled_task:updated', { id: 4, name: 'D', status: null })
    expect(events.recentEvents.map((event) => (event.data as { id: number }).id)).toEqual([3, 2])
  })
})
