import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { installAreaRealtime } from '@/areas/realtime'
import { apiService } from '@/services/apiService'
import { useDeployBuildsStore } from '@/stores/deployBuilds'
import { useEventsStore } from '@/stores/events'
import { useRegistryStore } from '@/stores/registry'
import type { DeployServerBuildView } from '@/bindings/DeployServerBuildView'
import type { RegistryOverview } from '@/bindings/RegistryOverview'

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
    data: JSON.stringify({ type, timestamp: '2026-11-09T12:00:00Z', data }),
  } as MessageEvent<string>)
}

function build(
  id: number,
  status: DeployServerBuildView['status'],
  extra: Partial<DeployServerBuildView> = {}
): DeployServerBuildView {
  return {
    id,
    applicationId: 1,
    application: 'NumBlaster',
    reference: 'refs/heads/main',
    commitSha: 'b'.repeat(40),
    commitMessage: 'feat: sala nova',
    trigger: 'push',
    triggeredBy: 'maxuel',
    agent: 'central',
    status,
    stage: null,
    currentImage: null,
    noCache: false,
    images: [],
    error: null,
    startedAt: null,
    finishedAt: null,
    durationMs: null,
    createdAt: '2026-11-09T12:00:00Z',
    ...extra,
  }
}

function registryOverview(): RegistryOverview {
  return {
    settings: { keepVersions: 10, quotaGib: 50 },
    usedBytes: 0,
    quotaBytes: 50 * 1024 ** 3,
    repositories: [],
    feeds: [],
  }
}

describe('SSE da compilação no servidor e do registry', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    installAreaRealtime()
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

  it('o estado e o console da compilação chegam pelo SSE, sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const events = useEventsStore()
    const store = useDeployBuildsStore()
    store.consoles = { 3: [] }
    events.connect()

    emit('deploys:build', build(3, 'queued'))
    expect(store.latestOf(1)?.status).toBe('queued')

    emit('deploys:build', build(3, 'running', { stage: 'build', currentImage: 'api' }))
    emit('deploys:build-log', { buildId: 3, lines: ['#5 [build 2/4] RUN npm ci', '#5 DONE 12.3s'] })
    emit('deploys:build-log', { buildId: 99, lines: ['console fechado: ignorado'] })
    expect(store.consoles[3]).toEqual(['#5 [build 2/4] RUN npm ci', '#5 DONE 12.3s'])
    expect(store.consoles[99]).toBeUndefined()
    expect(store.builds[3]?.currentImage).toBe('api')

    emit('deploys:build', build(3, 'succeeded', { durationMs: 42000 }))
    // Payload estranho é ignorado; um "compilando" atrasado não desfaz o fim.
    emit('deploys:build', { qualquer: 1 })
    emit('deploys:build', build(3, 'running'))
    expect(store.builds[3]?.status).toBe('succeeded')

    expect(get).not.toHaveBeenCalled()
    events.disconnect()
  })

  it('registry:changed só relê a visão de quem abriu a página', () => {
    vi.useFakeTimers()
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(registryOverview())
    const events = useEventsStore()
    const registry = useRegistryStore()
    events.connect()

    emit('registry:changed', {})
    vi.runAllTimers()
    expect(get).not.toHaveBeenCalled()

    registry.overview = registryOverview()
    emit('registry:changed', {})
    emit('registry:changed', {})
    vi.runAllTimers()
    expect(get).toHaveBeenCalledTimes(1)
    expect(get).toHaveBeenCalledWith('/registry')
    events.disconnect()
    vi.useRealTimers()
  })
})
