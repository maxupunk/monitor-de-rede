import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'

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

describe('events store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    FakeEventSource.latest = null
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

  it('aplica as métricas do snapshot monitor:result sem consultar endpoints', () => {
    const apiGet = vi.spyOn(apiService, 'get')
    const detail = useDeviceDetailStore()
    detail.device = { id: 7 } as never
    const events = useEventsStore()
    const metricHandler = vi.fn()
    events.onEvent('metric:recorded', metricHandler)
    events.connect()

    FakeEventSource.latest?.onmessage?.({
      data: JSON.stringify({
        type: 'monitor:result',
        timestamp: '2026-09-01T12:00:00Z',
        data: {
          deviceId: 7,
          monitorId: 11,
          status: 'up',
          recordedAt: '2026-09-01T12:00:00Z',
          metrics: [
            {
              name: 'memory_usage',
              value: 42,
              unit: 'percent',
              recordedAt: '2026-09-01T12:00:00Z',
            },
          ],
        },
      }),
    } as MessageEvent<string>)

    expect(detail.metrics).toEqual([
      expect.objectContaining({
        deviceId: 7,
        metricName: 'memory_usage',
        metricValue: 42,
        createdAt: '2026-09-01T12:00:00Z',
      }),
    ])
    expect(metricHandler).toHaveBeenCalledOnce()
    expect(apiGet).not.toHaveBeenCalled()
    events.disconnect()
  })
})
