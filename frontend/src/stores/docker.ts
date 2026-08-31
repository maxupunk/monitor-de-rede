import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { dockerService } from '@/services/dockerService'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerImageSummary } from '@/bindings/DockerImageSummary'
import type { DockerMetricsResponse } from '@/bindings/DockerMetricsResponse'
import type { DockerNetworkSummary } from '@/bindings/DockerNetworkSummary'
import type { DockerStatusResponse } from '@/bindings/DockerStatusResponse'
import type { DockerVolumeSummary } from '@/bindings/DockerVolumeSummary'

export const useDockerStore = defineStore('docker', () => {
  const status = ref<DockerStatusResponse | null>(null)
  const containers = ref<DockerContainerSummary[]>([])
  const volumes = ref<DockerVolumeSummary[]>([])
  const networks = ref<DockerNetworkSummary[]>([])
  const images = ref<DockerImageSummary[]>([])
  const metrics = ref<DockerMetricsResponse | null>(null)
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

  async function refreshAll(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      status.value = await dockerService.status()
      if (!status.value.available) {
        containers.value = []
        volumes.value = []
        networks.value = []
        images.value = []
        metrics.value = null
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
    } catch (reason: unknown) {
      fail(reason, 'Erro ao atualizar containers')
    }
  }

  async function runAction(action: () => Promise<unknown>, refresh = refreshAll): Promise<boolean> {
    actionLoading.value = true
    error.value = null
    try {
      await action()
      await refresh()
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
    loading,
    actionLoading,
    error,
    available,
    runningContainers,
    stoppedContainers,
    refreshAll,
    refreshContainers,
    runAction,
  }
})
