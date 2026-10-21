import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useEventsStore } from '@/stores/events'
import { useLinkTestsStore } from '@/stores/linkTests'
import { test } from '../utils/linkTests/fixtures'

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
    data: JSON.stringify({ type, timestamp: '2026-10-16T12:00:00Z', data }),
  } as MessageEvent<string>)
}

describe('SSE do teste de enlace', () => {
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

  it('roteia os quatro eventos para o store, sem consultar a API e fora do feed', () => {
    const get = vi.spyOn(apiService, 'get')
    const events = useEventsStore()
    const store = useLinkTestsStore()
    events.connect()

    // Snapshot inicial dos testes ativos ao conectar.
    emit('link_test:updated', test(11, { status: 'running' }))
    emit('link_test:sample', { testId: 11, legSeq: 1, flow: 'forward', bps: 4.8e8, tMs: 1000 })
    emit('link_test:health', {
      testId: 11,
      legSeq: 1,
      deviceId: 2,
      atMs: 1000,
      cpu: 93,
      memory: 61,
      phase: 'during',
    })
    // Repetida (reconexão) e payload inválido: ignorados.
    emit('link_test:health', {
      testId: 11,
      legSeq: 1,
      deviceId: 2,
      atMs: 1000,
      cpu: 10,
      memory: 61,
      phase: 'during',
    })
    emit('link_test:health', { testId: 11, legSeq: 1, deviceId: 2, atMs: 2000, cpu: 'alto' })
    emit('link_test_schedule:updated', { id: 3 })

    expect(store.active.map((item) => item.id)).toEqual([11])
    expect(store.liveBps(11, 1, 'forward')).toBe(4.8e8)
    expect(store.healthFor(11, 1)).toEqual([
      { deviceId: 2, atMs: 1000, cpu: 93, memory: 61, phase: 'during' },
    ])
    // Lista de agendamentos nunca aberta: nada a reler.
    expect(get).not.toHaveBeenCalled()
    expect(events.recentEvents.map((event) => event.type)).toEqual(['link_test_schedule:updated'])
  })

  it('agendamento atualizado relê a lista aberta', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue([])
    const store = useLinkTestsStore()
    await store.fetchSchedules()
    useEventsStore().connect()
    emit('link_test_schedule:updated', { id: 3 })
    expect(get).toHaveBeenCalledTimes(2)
    expect(get).toHaveBeenLastCalledWith('/link-test-schedules')
  })
})
