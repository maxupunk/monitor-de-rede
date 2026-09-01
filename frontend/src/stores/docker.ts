import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { dockerService } from '@/services/dockerService'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerImageSummary } from '@/bindings/DockerImageSummary'
import type { DockerInventorySnapshot } from '@/bindings/DockerInventorySnapshot'
import type { DockerLiveSnapshot } from '@/bindings/DockerLiveSnapshot'
import type { DockerMetricsResponse } from '@/bindings/DockerMetricsResponse'
import type { DockerNetworkSummary } from '@/bindings/DockerNetworkSummary'
import type { DockerStatusResponse } from '@/bindings/DockerStatusResponse'
import type { DockerVolumeSummary } from '@/bindings/DockerVolumeSummary'

export interface DockerAggregateSample {
  recordedAt: string
  cpuPercent: number
  memoryPercent: number
  memoryUsageBytes: number
  networkReceivedBytes: number
  networkTransmittedBytes: number
}

const HISTORY_LIMIT = 60

export const useDockerStore = defineStore('docker', () => {
  const status = ref<DockerStatusResponse | null>(null)
  const containers = ref<DockerContainerSummary[]>([])
  const volumes = ref<DockerVolumeSummary[]>([])
  const networks = ref<DockerNetworkSummary[]>([])
  const images = ref<DockerImageSummary[]>([])
  const metrics = ref<DockerMetricsResponse | null>(null)
  const aggregateHistory = ref<DockerAggregateSample[]>([])
  const lastUpdatedAt = ref<string | null>(null)
  const loading = ref(false)
  const actionLoading = ref(false)
  const error = ref<string | null>(null)

  const available = computed(() => status.value?.available === true)
  const runningContainers = computed(
    () => containers.value.filter((container) => container.state === 'running').length
  )
  const stoppedContainers = computed(() => containers.value.length - runningContainers.value)

  function fail(reason: unknown, fallback: string): void {
    error.value = reason instanceof Error ? reason.message : fallback
  }

  function clearUnavailableState(): void {
    containers.value = []
    volumes.value = []
    networks.value = []
    images.value = []
    metrics.value = null
    aggregateHistory.value = []
  }

  function recordAggregate(response: DockerMetricsResponse): void {
    if (!response.dockerAvailable) return
    if (aggregateHistory.value.at(-1)?.recordedAt === response.collectedAt) return

    const memoryUsageBytes = response.containers.reduce(
      (total, container) => total + container.memory.usageBytes,
      0
    )
    const memoryTotalBytes = Math.max(status.value?.memoryTotalBytes ?? 0, 0)
    const sample: DockerAggregateSample = {
      recordedAt: response.collectedAt,
      cpuPercent: response.containers.reduce(
        (total, container) => total + container.cpu.usagePercent,
        0
      ),
      memoryPercent:
        memoryTotalBytes > 0 ? Math.min(100, (memoryUsageBytes / memoryTotalBytes) * 100) : 0,
      memoryUsageBytes,
      networkReceivedBytes: response.containers.reduce(
        (total, container) => total + container.network.receivedBytes,
        0
      ),
      networkTransmittedBytes: response.containers.reduce(
        (total, container) => total + container.network.transmittedBytes,
        0
      ),
    }
    aggregateHistory.value = [...aggregateHistory.value, sample].slice(-HISTORY_LIMIT)
    lastUpdatedAt.value = response.collectedAt
  }

  function applyLiveSnapshot(snapshot: DockerLiveSnapshot): void {
    status.value = snapshot.status
    if (!snapshot.status.available) {
      clearUnavailableState()
      loading.value = false
      error.value = null
      return
    }
    containers.value = snapshot.containers
    metrics.value = snapshot.metrics
    recordAggregate(snapshot.metrics)
    loading.value = false
    error.value = null
  }

  function applyInventorySnapshot(snapshot: DockerInventorySnapshot): void {
    volumes.value = snapshot.volumes
    networks.value = snapshot.networks
    images.value = snapshot.images
    error.value = null
  }

  async function refreshAll(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      status.value = await dockerService.status()
      if (!status.value.available) {
        clearUnavailableState()
        return
      }

      const [containerResult, volumeResult, networkResult, imageResult, metricsResult] =
        await Promise.all([
          dockerService.containers(),
          dockerService.volumes(),
          dockerService.networks(),
          dockerService.images(),
          dockerService.metrics(),
        ])
      containers.value = containerResult.data
      volumes.value = volumeResult.data
      networks.value = networkResult.data
      images.value = imageResult.data
      metrics.value = metricsResult
      recordAggregate(metricsResult)
    } catch (reason: unknown) {
      fail(reason, 'Erro ao consultar a Docker Engine')
    } finally {
      loading.value = false
    }
  }

  async function refreshContainers(): Promise<void> {
    try {
      const result = await dockerService.containers()
      containers.value = result.data
      status.value = await dockerService.status()
      metrics.value = await dockerService.metrics()
      recordAggregate(metrics.value)
    } catch (reason: unknown) {
      fail(reason, 'Erro ao atualizar containers')
    }
  }

  async function runAction(
    action: () => Promise<unknown>,
    refresh?: () => Promise<unknown>
  ): Promise<boolean> {
    actionLoading.value = true
    error.value = null
    try {
      await action()
      if (refresh) await refresh()
      return true
    } catch (reason: unknown) {
      fail(reason, 'Operação Docker não concluída')
      return false
    } finally {
      actionLoading.value = false
    }
  }

  return {
    status,
    containers,
    volumes,
    networks,
    images,
    metrics,
    aggregateHistory,
    lastUpdatedAt,
    loading,
    actionLoading,
    error,
    available,
    runningContainers,
    stoppedContainers,
    applyLiveSnapshot,
    applyInventorySnapshot,
    refreshAll,
    refreshContainers,
    runAction,
  }
})
