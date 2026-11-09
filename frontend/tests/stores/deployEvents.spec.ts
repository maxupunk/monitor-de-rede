import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { installAreaRealtime } from '@/areas/realtime'
import { apiService } from '@/services/apiService'
import { useDeploysStore } from '@/stores/deploys'
import { useEventsStore } from '@/stores/events'
import type { DeployOverview } from '@/bindings/DeployOverview'
import type { DeployRunView } from '@/bindings/DeployRunView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'

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
    data: JSON.stringify({ type, timestamp: '2026-11-08T12:00:00Z', data }),
  } as MessageEvent<string>)
}

function run(id: number, status: DeployRunView['status'], extra: Partial<DeployRunView> = {}) {
  return {
    id,
    applicationId: 1,
    applicationName: 'SGCv4',
    repository: 'master/sgcv4',
    targetId: 2,
    targetName: 'Produção',
    trigger: 'webhook',
    triggeredBy: 'maxuel',
    reference: 'main',
    commitSha: 'a'.repeat(40),
    shortSha: 'aaaaaaa',
    commitMessage: 'feat: NF-e',
    imageTag: 'a'.repeat(40),
    envVersionId: null,
    status,
    stage: null,
    rollbackOfRunId: null,
    afterRunId: null,
    backupId: null,
    approvedBy: null,
    approvedAt: null,
    scheduledFor: null,
    error: null,
    createdAt: '2026-11-08T12:00:00Z',
    startedAt: null,
    finishedAt: null,
    durationMs: null,
    ...extra,
  } satisfies DeployRunView
}

function target(): DeployTargetView {
  return {
    id: 2,
    applicationId: 1,
    name: 'Produção',
    agent: { id: 5, name: 'debian-prod', connected: true, allowsDeploy: true },
    project: 'sgc',
    reference: 'main',
    requiresApproval: true,
    windowStart: null,
    windowEnd: null,
    backupConnectionId: null,
    backupConnectionName: null,
    waitTimeoutSecs: 180,
    autoRollback: true,
    keepImages: 3,
    enabled: true,
    position: 0,
    live: null,
    pending: null,
    env: null,
  }
}

function overview(): DeployOverview {
  return {
    applications: [
      {
        id: 1,
        name: 'SGCv4',
        slug: 'sgc',
        repository: 'master/sgcv4',
        repositoryUrl: 'https://github.com/master/sgcv4.git',
        credentialId: null,
        credentialName: null,
        composeFiles: ['docker-compose.yml'],
        imageTagVariable: 'IMAGE_TAG',
        images: [],
        rollout: 'sequential',
        targets: [target()],
        createdAt: '2026-11-08T10:00:00Z',
      },
    ],
    recentRuns: [],
    counts: {
      applications: 1,
      targets: 1,
      succeeded24h: 0,
      failed24h: 0,
      awaitingApproval: 0,
      running: 0,
    },
    credentials: [],
    hookPath: '/api/deploy-hooks/github',
    oidcAudience: 'netmonitor-deploy',
  }
}

describe('SSE do deploy contínuo', () => {
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

  it('o estado e o console chegam pelo SSE, sem consultar a API', () => {
    const get = vi.spyOn(apiService, 'get')
    const events = useEventsStore()
    const store = useDeploysStore()
    store.overview = overview()
    store.consoles = { 7: [] }
    events.connect()

    emit('deploys:run', run(7, 'awaiting_approval'))
    expect(store.targets[0]?.pending?.id).toBe(7)
    expect(store.awaitingApproval.map((item) => item.id)).toEqual([7])

    emit('deploys:run', run(7, 'running', { stage: 'pull' }))
    emit('deploys:log', { runId: 7, lines: ['Pulling sgc-api', 'sgc-api Pulled'] })
    emit('deploys:log', { runId: 99, lines: ['console fechado: ignorado'] })
    expect(store.consoles[7]).toEqual(['Pulling sgc-api', 'sgc-api Pulled'])
    expect(store.consoles[99]).toBeUndefined()

    emit('deploys:run', run(7, 'succeeded', { finishedAt: '2026-11-08T12:01:00Z' }))
    expect(store.targets[0]?.pending).toBeNull()
    expect(store.targets[0]?.live?.id).toBe(7)

    // Payload estranho é ignorado; um "rodando" atrasado não desfaz o fim.
    emit('deploys:run', { qualquer: 1 })
    emit('deploys:run', run(7, 'running'))
    expect(store.runById(7)?.status).toBe('succeeded')

    expect(get).not.toHaveBeenCalled()
    events.disconnect()
  })

  it('deploys:updated só relê a visão geral de quem abriu a página', () => {
    vi.useFakeTimers()
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(overview())
    const events = useEventsStore()
    const store = useDeploysStore()
    events.connect()

    emit('deploys:updated', {})
    vi.runAllTimers()
    expect(get).not.toHaveBeenCalled()

    store.overview = overview()
    emit('deploys:updated', {})
    emit('deploys:updated', {})
    vi.runAllTimers()
    expect(get).toHaveBeenCalledTimes(1)
    expect(get).toHaveBeenCalledWith('/deploys')
    events.disconnect()
    vi.useRealTimers()
  })
})
