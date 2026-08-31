import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useDockerStore } from '@/stores/docker'
import { dockerService } from '@/services/dockerService'

vi.mock('@/services/dockerService', () => ({
  dockerService: {
    status: vi.fn(),
    containers: vi.fn(),
    volumes: vi.fn(),
    networks: vi.fn(),
    images: vi.fn(),
    metrics: vi.fn(),
  },
}))

const mocked = vi.mocked(dockerService)

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

    await store.refreshAll()

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
        },
      ],
    })
    mocked.volumes.mockResolvedValue({ available: true, data: [] })
    mocked.networks.mockResolvedValue({ available: true, data: [] })
    mocked.images.mockResolvedValue({ available: true, data: [] })
    mocked.metrics.mockResolvedValue({
      dockerAvailable: true,
      unavailableReason: null,
      collectedAt: new Date().toISOString(),
      containers: [],
    })
    const store = useDockerStore()

    await store.refreshAll()

    expect(store.available).toBe(true)
    expect(store.runningContainers).toBe(1)
    expect(mocked.metrics).toHaveBeenCalledOnce()
    expect(store.loading).toBe(false)
  })
})
