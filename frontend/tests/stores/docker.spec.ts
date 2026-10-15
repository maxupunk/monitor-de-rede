import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useDockerStore } from '@/stores/docker'
import { createDockerService, dockerHostsService } from '@/services/dockerService'
import type { DockerHostView } from '@/bindings/DockerHostView'

/** O mesmo serviço falso para qualquer host: o que importa é a chave usada. */
const hostApi = vi.hoisted(() => ({
  status: vi.fn(),
  containers: vi.fn(),
  volumes: vi.fn(),
  networks: vi.fn(),
  images: vi.fn(),
  metrics: vi.fn(),
  followLogs: vi.fn(),
}))

vi.mock('@/services/dockerService', () => ({
  createDockerService: vi.fn(() => hostApi),
  dockerHostsService: {
    hosts: vi.fn(),
    stopLogStream: vi.fn(),
  },
}))

const mocked = hostApi

function host(key: string, kind: 'local' | 'agent', name: string): DockerHostView {
  return {
    key,
    name,
    kind,
    online: true,
    agentId: Number(key.replace('agent-', '')),
    dockerAvailable: true,
    composeAvailable: false,
    policy: ['read', 'lifecycle'],
  }
}

describe('docker store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('distingue Engine indisponível de inventário vazio', async () => {
    mocked.status.mockResolvedValue({
      available: false,
      reason: 'socket ausente',
      engineVersion: null,
      apiVersion: null,
      name: null,
      operatingSystem: null,
      architecture: null,
      cpus: null,
      memoryTotalBytes: null,
      containers: null,
      containersRunning: null,
      containersStopped: null,
      images: null,
    })
    const store = useDockerStore()
    store.selectHost('agent-1')

    await store.refreshAll()

    expect(createDockerService).toHaveBeenLastCalledWith('agent-1')
    expect(store.available).toBe(false)
    expect(mocked.containers).not.toHaveBeenCalled()
    expect(store.error).toBeNull()
  })

  it('carrega as cinco coleções em paralelo quando a Engine responde', async () => {
    mocked.status.mockResolvedValue({
      available: true,
      reason: null,
      engineVersion: '28.0',
      apiVersion: '1.47',
      name: 'docker-host',
      operatingSystem: 'Linux',
      architecture: 'x86_64',
      cpus: 8,
      memoryTotalBytes: 1024,
      containers: 1,
      containersRunning: 1,
      containersStopped: 0,
      images: 1,
    })
    mocked.containers.mockResolvedValue({
      available: true,
      data: [
        {
          id: 'abc',
          names: ['/web'],
          image: 'nginx',
          imageId: 'def',
          state: 'running',
          status: 'Up',
          labels: {},
          ports: [],
          created: 1,
          projectName: null,
          protected: false,
        },
      ],
    })
    mocked.volumes.mockResolvedValue({ available: true, data: [] })
    mocked.networks.mockResolvedValue({ available: true, data: [] })
    mocked.images.mockResolvedValue({ available: true, data: [] })
    mocked.metrics.mockResolvedValue({
      dockerAvailable: true,
      unavailableReason: null,
      failedContainerCount: 0,
      collectedAt: '2026-08-31T12:00:00Z',
      containers: [
        {
          containerId: 'abc',
          containerName: 'web',
          projectName: null,
          imageName: 'nginx',
          status: 'running',
          cpu: { usagePercent: 12.5 },
          memory: { usageBytes: 256, limitBytes: 1024, usagePercent: 25 },
          network: { receivedBytes: 100, transmittedBytes: 50 },
          blockIo: { readBytes: 10, writeBytes: 5 },
          pids: 2,
        },
      ],
    })
    const store = useDockerStore()
    store.selectHost('agent-1')

    await store.refreshAll()

    expect(store.available).toBe(true)
    expect(store.runningContainers).toBe(1)
    expect(mocked.metrics).toHaveBeenCalledOnce()
    expect(store.aggregateHistory).toEqual([
      {
        recordedAt: '2026-08-31T12:00:00Z',
        cpuPercent: 12.5,
        memoryPercent: 25,
        memoryUsageBytes: 256,
        networkReceivedBytes: 100,
        networkTransmittedBytes: 50,
        cpuByContainer: { abc: 12.5 },
        memoryByContainer: { abc: 256 },
        containerNames: { abc: 'web' },
      },
    ])
    expect(store.loading).toBe(false)
  })

  it('sem host escolhido não consulta nada', async () => {
    const store = useDockerStore()

    await store.refreshAll()
    await store.refreshContainers()

    expect(mocked.status).not.toHaveBeenCalled()
    expect(store.loading).toBe(false)
  })

  it('abre o agente local por padrão e o expõe ao dashboard', async () => {
    vi.mocked(dockerHostsService).hosts.mockResolvedValue([
      host('agent-1', 'local', 'Este servidor'),
      host('agent-7', 'agent', 'srv-remoto'),
    ])
    const store = useDockerStore()
    await store.fetchHosts()

    expect(store.hostsLoaded).toBe(true)
    expect(store.selectedHostKey).toBe('agent-1')
    expect(store.defaultHostKey).toBe('agent-1')

    store.selectHost('agent-7')
    store.applyLiveSnapshot(liveSnapshot('agent-1', 'central', ['db']))
    expect(store.localView.containers).toHaveLength(1)
    expect(store.containers).toHaveLength(0)
  })

  it('aplica snapshots SSE sem consultar os endpoints Docker', () => {
    const store = useDockerStore()
    store.selectHost('agent-1')

    store.applyLiveSnapshot({
      hostKey: 'agent-1',
      status: {
        available: true,
        reason: null,
        engineVersion: '28.0',
        apiVersion: '1.47',
        name: 'docker-host',
        operatingSystem: 'Linux',
        architecture: 'x86_64',
        cpus: 8,
        memoryTotalBytes: 1024,
        containers: 1,
        containersRunning: 1,
        containersStopped: 0,
        images: 1,
      },
      containers: [
        {
          id: 'abc',
          names: ['/web'],
          image: 'nginx',
          imageId: 'def',
          state: 'running',
          status: 'Up',
          labels: {},
          ports: [],
          created: 1,
          projectName: 'app',
          protected: false,
        },
      ],
      metrics: {
        dockerAvailable: true,
        unavailableReason: null,
        failedContainerCount: 0,
        collectedAt: '2026-08-31T12:00:03Z',
        containers: [],
      },
    })
    store.applyInventorySnapshot({
      hostKey: 'agent-1',
      collectedAt: '2026-08-31T12:00:03Z',
      volumes: [],
      networks: [],
      images: [],
    })

    expect(store.available).toBe(true)
    expect(store.runningContainers).toBe(1)
    expect(mocked.status).not.toHaveBeenCalled()
    expect(mocked.containers).not.toHaveBeenCalled()
    expect(mocked.metrics).not.toHaveBeenCalled()
  })

  it('mantém o estado de cada host separado e mostra o selecionado', () => {
    const store = useDockerStore()
    store.selectHost('agent-1')
    store.applyLiveSnapshot(liveSnapshot('agent-7', 'srv-remoto', ['web', 'api']))
    store.applyLiveSnapshot(liveSnapshot('agent-1', 'central', ['db']))

    expect(store.containers).toHaveLength(1)
    expect(store.status?.name).toBe('central')

    store.selectHost('agent-7')
    expect(store.status?.name).toBe('srv-remoto')
    expect(store.containers.map((container) => container.id)).toEqual(['web', 'api'])
    expect(store.hostView('agent-1').containers).toHaveLength(1)
    expect(mocked.containers).not.toHaveBeenCalled()
  })

  it('probe:status do agente muda o host para offline sem nova consulta', async () => {
    const hostsService = vi.mocked(dockerHostsService)
    hostsService.hosts.mockResolvedValue([host('agent-7', 'agent', 'srv-remoto')])
    const store = useDockerStore()
    await store.fetchHosts()
    store.selectHost('agent-7')

    store.applyAgentStatus({ id: 7, status: 'offline' })

    expect(store.selectedHost?.online).toBe(false)
    expect(store.allows('lifecycle')).toBe(true)
    expect(store.allows('update')).toBe(false)
    expect(hostsService.hosts).toHaveBeenCalledOnce()
  })

  it('acumula o progresso de uma operação pelo SSE', () => {
    const store = useDockerStore()
    const base = {
      operationId: 'op-1',
      hostKey: 'agent-7',
      kind: 'update',
      target: 'web',
      message: null,
    }
    store.applyOperationEvent({ ...base, state: 'running', line: null })
    store.applyOperationEvent({ ...base, state: 'running', line: 'Baixando nginx:alpine…' })
    store.applyOperationEvent({
      ...base,
      state: 'succeeded',
      line: null,
      message: 'web recriado',
    })

    expect(store.operations).toHaveLength(1)
    expect(store.operations[0].lines).toEqual(['Baixando nginx:alpine…'])
    expect(store.operations[0].state).toBe('succeeded')
    expect(store.operations[0].message).toBe('web recriado')
  })

  it('logs ao vivo chegam pelo SSE depois de uma única requisição de início', async () => {
    mocked.followLogs.mockResolvedValue({ streamId: 's-1' })
    const store = useDockerStore()
    store.selectHost('agent-1')

    const streamId = await store.startLogStream('web', 100)
    store.applyLogStreamEvent({
      streamId,
      hostKey: 'agent-1',
      containerId: 'web',
      entries: [{ timestamp: '2026-09-22T10:00:00Z', stream: 'stdout', message: 'pronto' }],
      ended: null,
    })
    store.applyLogStreamEvent({
      streamId,
      hostKey: 'agent-1',
      containerId: 'web',
      entries: [],
      ended: 'Acompanhamento encerrado',
    })

    expect(mocked.followLogs).toHaveBeenCalledOnce()
    expect(store.logStreams[streamId].entries.map((entry) => entry.message)).toEqual(['pronto'])
    expect(store.logStreams[streamId].ended).toBe('Acompanhamento encerrado')

    await store.stopLogStream(streamId)
    expect(store.logStreams[streamId]).toBeUndefined()
    expect(vi.mocked(dockerHostsService).stopLogStream).toHaveBeenCalledWith('s-1')
  })
})

function liveSnapshot(hostKey: string, name: string, ids: string[]) {
  return {
    hostKey,
    status: {
      available: true,
      reason: null,
      engineVersion: '28.0',
      apiVersion: '1.47',
      name,
      operatingSystem: 'Linux',
      architecture: 'x86_64',
      cpus: 4,
      memoryTotalBytes: 1024,
      containers: ids.length,
      containersRunning: ids.length,
      containersStopped: 0,
      images: 1,
    },
    containers: ids.map((id) => ({
      id,
      names: ['/' + id],
      image: 'nginx',
      imageId: 'img',
      state: 'running',
      status: 'Up',
      labels: {},
      ports: [],
      created: 1,
      projectName: null,
      protected: false,
    })),
    metrics: {
      dockerAvailable: true,
      unavailableReason: null,
      failedContainerCount: 0,
      collectedAt: '2026-09-22T10:00:0' + ids.length + 'Z',
      containers: [],
    },
  }
}
