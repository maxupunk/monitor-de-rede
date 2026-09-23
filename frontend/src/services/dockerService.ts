import { apiService } from '@/services/apiService'
import type { ComposeAction } from '@/bindings/ComposeAction'
import type { ComposeProject } from '@/bindings/ComposeProject'
import type { ContainerHistoryResponse } from '@/bindings/ContainerHistoryResponse'
import type { DockerActionResponse } from '@/bindings/DockerActionResponse'
import type { DockerContainerDetail } from '@/bindings/DockerContainerDetail'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerHostView } from '@/bindings/DockerHostView'
import type { DockerImageDetail } from '@/bindings/DockerImageDetail'
import type { DockerImageSummary } from '@/bindings/DockerImageSummary'
import type { DockerLogEntry } from '@/bindings/DockerLogEntry'
import type { DockerLogStreamStarted } from '@/bindings/DockerLogStreamStarted'
import type { DockerMetricsResponse } from '@/bindings/DockerMetricsResponse'
import type { DockerNetworkDetail } from '@/bindings/DockerNetworkDetail'
import type { DockerNetworkSummary } from '@/bindings/DockerNetworkSummary'
import type { DockerOperationAccepted } from '@/bindings/DockerOperationAccepted'
import type { DockerPruneResponse } from '@/bindings/DockerPruneResponse'
import type { DockerStatusResponse } from '@/bindings/DockerStatusResponse'
import type { DockerVolumeDetail } from '@/bindings/DockerVolumeDetail'
import type { DockerVolumeSummary } from '@/bindings/DockerVolumeSummary'
import type { HostHistoryResponse } from '@/bindings/HostHistoryResponse'
import type { MetricsRange } from '@/bindings/MetricsRange'

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

/** Host da própria central; os demais são `agent-<id>` (ADR 011). */
export const LOCAL_HOST_KEY = 'local'

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

/**
 * Endpoints Docker de um host. O host local mantém as rotas históricas
 * (`/docker/...`); os remotos vão por `/docker/hosts/<chave>/...`.
 */
export function createDockerService(hostKey: string = LOCAL_HOST_KEY) {
  const base =
    hostKey === LOCAL_HOST_KEY ? '/docker' : `/docker/hosts/${encodeURIComponent(hostKey)}`

  function resource(path: string): string {
    return `${base}${path}`
  }

  return {
    hostKey,

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

    /** Inicia o acompanhamento; as linhas chegam pelo SSE como `docker:log`. */
    followLogs(id: string, tail: number | 'all' = 100): Promise<DockerLogStreamStarted> {
      return apiService.post(resource(`/containers/${encodeURIComponent(id)}/logs/follow`), {
        tail: String(tail),
      })
    },

    clearLogs(id: string): Promise<DockerActionResponse> {
      return apiService.delete(resource(`/containers/${encodeURIComponent(id)}/logs`))
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

    /** Pull da tag e recriação com rollback; o progresso chega pelo SSE. */
    updateContainer(id: string): Promise<DockerOperationAccepted> {
      return apiService.post(resource(`/containers/${encodeURIComponent(id)}/update`))
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

    pullImage(image: string): Promise<DockerOperationAccepted> {
      return apiService.post(resource('/images/pull'), { image })
    },

    composeProjects(): Promise<DockerListing<ComposeProject>> {
      return apiService.get(resource('/compose'))
    },

    composeAction(
      project: string,
      action: ComposeAction,
      service?: string
    ): Promise<DockerOperationAccepted> {
      return apiService.post(resource(`/compose/${encodeURIComponent(project)}`), {
        action,
        service: service ?? null,
      })
    },

    history(range: MetricsRange): Promise<HostHistoryResponse> {
      return apiService.get(resource('/history') + query({ range }))
    },

    containerHistory(name: string, range: MetricsRange): Promise<ContainerHistoryResponse> {
      return apiService.get(
        resource(`/history/containers/${encodeURIComponent(name)}`) + query({ range })
      )
    },
  }
}

export type DockerService = ReturnType<typeof createDockerService>

/** Serviço do Docker desta central — compatível com os usos anteriores. */
export const dockerService = createDockerService(LOCAL_HOST_KEY)

/** Operações que não pertencem a um host. */
export const dockerHostsService = {
  hosts(): Promise<DockerHostView[]> {
    return apiService.get('/docker/hosts')
  },

  stopLogStream(streamId: string): Promise<{ stopped: boolean }> {
    return apiService.delete(`/docker/log-streams/${encodeURIComponent(streamId)}`)
  },
}
