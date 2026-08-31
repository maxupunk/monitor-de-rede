import { apiService } from '@/services/apiService'
import type { DockerActionResponse } from '@/bindings/DockerActionResponse'
import type { DockerContainerDetail } from '@/bindings/DockerContainerDetail'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerImageDetail } from '@/bindings/DockerImageDetail'
import type { DockerImageSummary } from '@/bindings/DockerImageSummary'
import type { DockerLogEntry } from '@/bindings/DockerLogEntry'
import type { DockerMetricsResponse } from '@/bindings/DockerMetricsResponse'
import type { DockerNetworkDetail } from '@/bindings/DockerNetworkDetail'
import type { DockerNetworkSummary } from '@/bindings/DockerNetworkSummary'
import type { DockerPruneResponse } from '@/bindings/DockerPruneResponse'
import type { DockerStatusResponse } from '@/bindings/DockerStatusResponse'
import type { DockerVolumeDetail } from '@/bindings/DockerVolumeDetail'
import type { DockerVolumeSummary } from '@/bindings/DockerVolumeSummary'

export interface DockerListing<T> {
  available: boolean
  data: T[]
}

export interface DockerLogFilters {
  tail?: number | 'all'
  since?: number
  until?: number
  timestamps?: boolean
}

function resource(path: string): string {
  return `/docker${path}`
}

function query(params: Record<string, string | number | boolean | undefined>): string {
  const values = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) values.set(key, String(value))
  }
  const serialized = values.toString()
  return serialized ? `?${serialized}` : ''
}

function triggerDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  URL.revokeObjectURL(url)
}

export const dockerService = {
  status(): Promise<DockerStatusResponse> {
    return apiService.get(resource('/status'))
  },

  metrics(): Promise<DockerMetricsResponse> {
    return apiService.get(resource('/metrics'))
  },

  containers(): Promise<DockerListing<DockerContainerSummary>> {
    return apiService.get(resource('/containers'))
  },

  container(id: string): Promise<DockerContainerDetail> {
    return apiService.get(resource(`/containers/${encodeURIComponent(id)}`))
  },

  logs(id: string, filters: DockerLogFilters = {}): Promise<DockerLogEntry[]> {
    return apiService.get(
      resource(`/containers/${encodeURIComponent(id)}/logs`) +
        query({
          tail: filters.tail,
          since: filters.since,
          until: filters.until,
          timestamps: filters.timestamps,
        })
    )
  },

  startContainer(id: string): Promise<DockerActionResponse> {
    return apiService.post(resource(`/containers/${encodeURIComponent(id)}/start`))
  },

  stopContainer(id: string): Promise<DockerActionResponse> {
    return apiService.post(resource(`/containers/${encodeURIComponent(id)}/stop`))
  },

  restartContainer(id: string): Promise<DockerActionResponse> {
    return apiService.post(resource(`/containers/${encodeURIComponent(id)}/restart`))
  },

  removeContainer(id: string, force = false): Promise<DockerActionResponse> {
    return apiService.delete(resource(`/containers/${encodeURIComponent(id)}`) + query({ force }))
  },

  volumes(): Promise<DockerListing<DockerVolumeSummary>> {
    return apiService.get(resource('/volumes'))
  },

  volume(name: string): Promise<DockerVolumeDetail> {
    return apiService.get(resource(`/volumes/${encodeURIComponent(name)}`))
  },

  removeVolume(name: string, force = false): Promise<DockerActionResponse> {
    return apiService.delete(resource(`/volumes/${encodeURIComponent(name)}`) + query({ force }))
  },

  async exportVolume(name: string): Promise<void> {
    const blob = await apiService.download(
      resource(`/volumes/${encodeURIComponent(name)}/export`),
      {
        timeoutMs: 30 * 60 * 1000,
      }
    )
    const safeName = name.replace(/[^a-zA-Z0-9_-]/g, '_')
    triggerDownload(blob, `volume-${safeName}-${new Date().toISOString().slice(0, 10)}.tar.gz`)
  },

  networks(): Promise<DockerListing<DockerNetworkSummary>> {
    return apiService.get(resource('/networks'))
  },

  network(id: string): Promise<DockerNetworkDetail> {
    return apiService.get(resource(`/networks/${encodeURIComponent(id)}`))
  },

  createNetwork(name: string, driver = 'bridge'): Promise<DockerActionResponse> {
    return apiService.post(resource('/networks'), { name, driver })
  },

  connectNetwork(networkId: string, containerId: string): Promise<DockerActionResponse> {
    return apiService.post(resource(`/networks/${encodeURIComponent(networkId)}/connect`), {
      containerId,
    })
  },

  disconnectNetwork(
    networkId: string,
    containerId: string,
    force = false
  ): Promise<DockerActionResponse> {
    return apiService.post(resource(`/networks/${encodeURIComponent(networkId)}/disconnect`), {
      containerId,
      force,
    })
  },

  removeNetwork(id: string): Promise<DockerActionResponse> {
    return apiService.delete(resource(`/networks/${encodeURIComponent(id)}`))
  },

  images(): Promise<DockerListing<DockerImageSummary>> {
    return apiService.get(resource('/images'))
  },

  image(id: string): Promise<DockerImageDetail> {
    return apiService.get(resource(`/images/${encodeURIComponent(id)}`))
  },

  removeImage(id: string, force = false): Promise<DockerActionResponse> {
    return apiService.delete(resource(`/images/${encodeURIComponent(id)}`) + query({ force }))
  },

  pruneImages(): Promise<DockerPruneResponse> {
    return apiService.post(resource('/images/prune'))
  },
}
