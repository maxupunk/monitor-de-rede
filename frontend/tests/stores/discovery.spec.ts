import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useDiscoveryStore } from '@/stores/discovery'
import { useEventsStore } from '@/stores/events'

class FakeEventSource {
  static latest: FakeEventSource | null = null
  static opened = 0
  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null

  constructor(_url: string) {
    FakeEventSource.latest = this
    FakeEventSource.opened += 1
  }

  close() {}
}

function emit(type: string, data: Record<string, unknown>) {
  FakeEventSource.latest?.onmessage?.({
    data: JSON.stringify({ type, timestamp: '2026-10-03T12:00:00Z', data }),
  } as MessageEvent<string>)
}

const scanFrame = (overrides: Record<string, unknown> = {}) => ({
  runId: 9,
  networkId: 2,
  status: 'running',
  phase: 'identify',
  progressCurrent: 190,
  progressTotal: 254,
  hosts: [
    { ipAddress: '10.0.0.10', confidence: 80, deviceType: 'printer', data: {} },
    { ipAddress: '10.0.0.9', confidence: 60, deviceType: 'iot', data: {} },
  ],
  logs: ['Varredura iniciada.'],
  error: null,
  startedAt: '2026-10-03T12:00:00Z',
  finishedAt: null,
  ...overrides,
})

describe('discovery store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    FakeEventSource.latest = null
    FakeEventSource.opened = 0
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

  it('aplica o snapshot discovery:scan do stream global sem consultar endpoints', () => {
    const apiGet = vi.spyOn(apiService, 'get')
    const discovery = useDiscoveryStore()
    const events = useEventsStore()
    events.connect()

    emit('discovery:scan', scanFrame())

    expect(discovery.scanning).toBe(true)
    expect(discovery.progressPercent).toBeCloseTo((190 / 254) * 100)
    // Ordenados por IP numérico, não por texto.
    expect(discovery.hosts.map((host) => host.ipAddress)).toEqual(['10.0.0.9', '10.0.0.10'])
    // O quadro é estado, não notícia: não entra no feed de eventos.
    expect(events.recentEvents).toHaveLength(0)

    emit('discovery:scan', scanFrame({ status: 'completed', phase: 'idle' }))
    expect(discovery.scanning).toBe(false)
    expect(discovery.progressPercent).toBe(100)

    expect(apiGet).not.toHaveBeenCalled()
    // Um stream só: a varredura não abre SSE próprio.
    expect(FakeEventSource.opened).toBe(1)
    events.disconnect()
  })

  it('ignora quadro malformado', () => {
    const discovery = useDiscoveryStore()
    discovery.applyScanSnapshot({ status: 'running' })
    expect(discovery.scan.status).toBe('idle')
  })

  it('mostra a varredura como em curso enquanto o pedido de início está em voo', async () => {
    let resolvePost: (value: { runId: number; status: string }) => void = () => {}
    vi.spyOn(apiService, 'post').mockImplementation(
      () => new Promise((resolve) => (resolvePost = resolve as typeof resolvePost))
    )
    const discovery = useDiscoveryStore()
    const started = discovery.startScan(2)
    expect(discovery.scanning).toBe(true)
    resolvePost({ runId: 9, status: 'running' })
    expect(await started).toBe(9)
    discovery.applyScanSnapshot(scanFrame({ status: 'completed' }))
    expect(discovery.scanning).toBe(false)
  })
})
