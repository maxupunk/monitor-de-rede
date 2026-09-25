import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'
import { useMonitorsStore } from '@/stores/monitors'
import { useAlertsStore } from '@/stores/alerts'
import { useDockerStore } from '@/stores/docker'
import { useTopologyStore } from '@/stores/topology'

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
    const monitors = useMonitorsStore()
    monitors.currentMonitor = {
      id: 11,
      deviceId: 7,
      name: 'OpenAI',
      type: 'https',
      target: 'https://chatgpt.com',
      intervalSeconds: 60,
      timeoutSeconds: 5,
      status: 'up',
      isEnabled: true,
    }
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
          adaptiveLatency: {
            applies: true,
            alertEligible: false,
            reason: 'collecting_confirmations',
            deviationPercent: 50,
            requiredConsecutiveChecks: 3,
            observedConsecutiveChecks: 2,
            expectedLatencyMs: 220,
            alertThresholdMs: 330,
            currentLatencyMs: 350,
            linkUtilizationPercent: 35,
            linkSaturated: false,
            sourceDeviceId: 7,
            linkInterfaceId: 3,
            linkInterfaceName: 'wan1',
            capacitySource: 'configured',
          },
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
    expect(monitors.currentMonitor?.adaptiveLatency).toEqual(
      expect.objectContaining({
        reason: 'collecting_confirmations',
        observedConsecutiveChecks: 2,
        currentLatencyMs: 350,
      })
    )
    expect(apiGet).not.toHaveBeenCalled()
    events.disconnect()
  })
  it('aplica o resumo da IA de alert:ai_summary sem consultar endpoints', () => {
    const apiGet = vi.spyOn(apiService, 'get')
    const alerts = useAlertsStore()
    alerts.upsertAlertEvent({
      id: 42,
      severity: 'critical',
      status: 'active',
      title: 'Borda fora do ar',
      message: 'Timeout',
      data: { problemKind: 'unreachable' },
      createdAt: '2026-09-21T10:00:00Z',
    })
    const events = useEventsStore()
    events.connect()

    FakeEventSource.latest?.onmessage?.({
      data: JSON.stringify({
        type: 'alert:ai_summary',
        timestamp: '2026-09-21T10:01:00Z',
        data: {
          id: 42,
          alertEventId: 42,
          aiSummary: {
            text: 'Uplink caiu; trocar o SFP.',
            generatedAt: '2026-09-21T10:01:00Z',
            promptTokens: 900,
            completionTokens: 30,
          },
        },
      }),
    } as MessageEvent<string>)

    const alerta = alerts.alertEvents.find((event) => event.id === 42)
    expect(alerta?.data?.aiSummary?.text).toBe('Uplink caiu; trocar o SFP.')
    expect(alerta?.data?.problemKind).toBe('unreachable')
    expect(apiGet).not.toHaveBeenCalled()
    events.disconnect()
  })

  it('roteia snapshot e operação Docker de host remoto sem consultar endpoints', () => {
    const apiGet = vi.spyOn(apiService, 'get')
    const events = useEventsStore()
    events.connect()
    const send = (type: string, data: unknown) =>
      FakeEventSource.latest?.onmessage?.({
        data: JSON.stringify({ type, timestamp: '2026-09-22T10:00:00Z', data }),
      } as MessageEvent<string>)

    send('docker:snapshot', {
      hostKey: 'agent-4',
      status: { available: true, reason: null, name: 'srv-4' },
      containers: [],
      metrics: {
        dockerAvailable: true,
        unavailableReason: null,
        failedContainerCount: 0,
        collectedAt: '2026-09-22T10:00:00Z',
        containers: [],
      },
    })
    send('docker:operation', {
      operationId: 'op-9',
      hostKey: 'agent-4',
      kind: 'pull',
      target: 'nginx:alpine',
      state: 'running',
      line: 'Pulling fs layer',
      message: null,
    })

    const docker = useDockerStore()
    expect(docker.hostView('agent-4').status?.name).toBe('srv-4')
    expect(docker.hostView('local').status).toBeNull()
    expect(docker.operations[0].lines).toEqual(['Pulling fs layer'])
    expect(events.recentEvents).toHaveLength(0)
    expect(apiGet).not.toHaveBeenCalled()
  })

  it('atualiza métricas de tráfego de topologia em memória via interface:traffic sem consultar endpoints', () => {
    const apiGet = vi.spyOn(apiService, 'get')
    const topology = useTopologyStore()
    topology.edges = [
      {
        id: '1-2-10-20',
        source: 1,
        target: 2,
        sourceInterfaceId: 10,
        targetInterfaceId: 20,
        sourceInterface: 'ge-0/0/1',
        targetInterface: 'eth1',
        status: 'unknown',
        trafficBps: 0,
        trafficLabel: undefined,
        inBps: 0,
        outBps: 0,
      } as never,
    ]

    const events = useEventsStore()
    events.connect()

    FakeEventSource.latest?.onmessage?.({
      data: JSON.stringify({
        type: 'interface:traffic',
        timestamp: '2026-09-24T22:00:00Z',
        data: {
          deviceId: 1,
          deviceName: 'Core-Router',
          interfaces: [
            {
              id: 10,
              name: 'ge-0/0/1',
              inBps: 15_000_000,
              outBps: 45_000_000,
              operStatus: 'up',
              adminStatus: 'up',
              speed: 1_000_000_000,
            },
          ],
        },
      }),
    } as MessageEvent<string>)

    expect(topology.edges[0].trafficBps).toBe(60_000_000)
    expect(topology.edges[0].inBps).toBe(15_000_000)
    expect(topology.edges[0].outBps).toBe(45_000_000)
    expect(topology.edges[0].trafficLabel).toBe('60 Mbps')
    expect(topology.edges[0].inBpsLabel).toBe('15 Mbps')
    expect(topology.edges[0].outBpsLabel).toBe('45 Mbps')
    expect(topology.edges[0].status).toBe('up')
    expect(events.recentEvents).toHaveLength(0) // Efêmero, não polui feed
    expect(apiGet).not.toHaveBeenCalled()
    events.disconnect()
  })
})
