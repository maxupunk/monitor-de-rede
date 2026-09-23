import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  createDockerService,
  dockerHostsService,
  dockerService,
  LOCAL_HOST_KEY,
  type DockerService,
} from '@/services/dockerService'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerHostView } from '@/bindings/DockerHostView'
import type { DockerImageSummary } from '@/bindings/DockerImageSummary'
import type { DockerInventorySnapshot } from '@/bindings/DockerInventorySnapshot'
import type { DockerLiveSnapshot } from '@/bindings/DockerLiveSnapshot'
import type { DockerLogEntry } from '@/bindings/DockerLogEntry'
import type { DockerLogStreamEvent } from '@/bindings/DockerLogStreamEvent'
import type { DockerMetricsResponse } from '@/bindings/DockerMetricsResponse'
import type { DockerNetworkSummary } from '@/bindings/DockerNetworkSummary'
import type { DockerOperationEvent } from '@/bindings/DockerOperationEvent'
import type { DockerStatusResponse } from '@/bindings/DockerStatusResponse'
import type { DockerVolumeSummary } from '@/bindings/DockerVolumeSummary'
import type { Permission } from '@/bindings/Permission'

export interface DockerAggregateSample {
  recordedAt: string
  cpuPercent: number
  memoryPercent: number
  memoryUsageBytes: number
  networkReceivedBytes: number
  networkTransmittedBytes: number
}

/** Estado de um host Docker (a central ou um agente), alimentado pelo SSE. */
export interface DockerHostState {
  status: DockerStatusResponse | null
  containers: DockerContainerSummary[]
  volumes: DockerVolumeSummary[]
  networks: DockerNetworkSummary[]
  images: DockerImageSummary[]
  metrics: DockerMetricsResponse | null
  aggregateHistory: DockerAggregateSample[]
  lastUpdatedAt: string | null
}

/** Operação longa (pull, update, compose) acompanhada pelo SSE. */
export interface DockerOperationState {
  operationId: string
  hostKey: string
  kind: string
  target: string
  state: string
  lines: string[]
  message: string | null
  updatedAt: string
}

export interface DockerLogStreamState {
  streamId: string
  hostKey: string
  containerId: string
  entries: DockerLogEntry[]
  ended: string | null
}

const HISTORY_LIMIT = 60
const OPERATION_LINES = 50
const OPERATIONS_KEPT = 20
const LOG_LINES_KEPT = 2000

function emptyHost(): DockerHostState {
  return {
    status: null,
    containers: [],
    volumes: [],
    networks: [],
    images: [],
    metrics: null,
    aggregateHistory: [],
    lastUpdatedAt: null,
  }
}

export const useDockerStore = defineStore('docker', () => {
  const hosts = ref<DockerHostView[]>([])
  const hostStates = ref<Record<string, DockerHostState>>({ [LOCAL_HOST_KEY]: emptyHost() })
  const selectedHostKey = ref<string>(LOCAL_HOST_KEY)
  const operations = ref<DockerOperationState[]>([])
  const logStreams = ref<Record<string, DockerLogStreamState>>({})
  const loading = ref(false)
  const actionLoading = ref(false)
  const error = ref<string | null>(null)

  function stateOf(hostKey: string): DockerHostState {
    if (!hostStates.value[hostKey]) {
      hostStates.value = { ...hostStates.value, [hostKey]: emptyHost() }
    }
    return hostStates.value[hostKey]
  }

  const current = computed<DockerHostState>(
    () => hostStates.value[selectedHostKey.value] ?? emptyHost()
  )
  const status = computed(() => current.value.status)
  const containers = computed(() => current.value.containers)
  const volumes = computed(() => current.value.volumes)
  const networks = computed(() => current.value.networks)
  const images = computed(() => current.value.images)
  const metrics = computed(() => current.value.metrics)
  const aggregateHistory = computed(() => current.value.aggregateHistory)
  const lastUpdatedAt = computed(() => current.value.lastUpdatedAt)

  const available = computed(() => status.value?.available === true)
  const runningContainers = computed(
    () => containers.value.filter((container) => container.state === 'running').length
  )
  const stoppedContainers = computed(() => containers.value.length - runningContainers.value)

  const selectedHost = computed<DockerHostView | null>(
    () => hosts.value.find((host) => host.key === selectedHostKey.value) ?? null
  )
  /** Serviço do host selecionado: as telas nunca montam caminho sozinhas. */
  const api = computed<DockerService>(() =>
    selectedHostKey.value === LOCAL_HOST_KEY
      ? dockerService
      : createDockerService(selectedHostKey.value)
  )

  /** O que a política do host selecionado libera (ADR 011). */
  function allows(permission: Permission): boolean {
    const host = selectedHost.value
    if (!host) return permission !== 'compose'
    return host.policy.includes(permission)
  }

  function hostView(hostKey: string): DockerHostState {
    return hostStates.value[hostKey] ?? emptyHost()
  }

  function fail(reason: unknown, fallback: string): void {
    error.value = reason instanceof Error ? reason.message : fallback
  }

  function clearUnavailableState(state: DockerHostState): void {
    state.containers = []
    state.volumes = []
    state.networks = []
    state.images = []
    state.metrics = null
    state.aggregateHistory = []
  }

  function recordAggregate(state: DockerHostState, response: DockerMetricsResponse): void {
    if (!response.dockerAvailable) return
    if (state.aggregateHistory.at(-1)?.recordedAt === response.collectedAt) return

    const memoryUsageBytes = response.containers.reduce(
      (total, container) => total + container.memory.usageBytes,
      0
    )
    const memoryTotalBytes = Math.max(state.status?.memoryTotalBytes ?? 0, 0)
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
    state.aggregateHistory = [...state.aggregateHistory, sample].slice(-HISTORY_LIMIT)
    state.lastUpdatedAt = response.collectedAt
  }

  function applyLiveSnapshot(snapshot: DockerLiveSnapshot): void {
    const state = stateOf(snapshot.hostKey || LOCAL_HOST_KEY)
    state.status = snapshot.status
    if (!snapshot.status.available) {
      clearUnavailableState(state)
    } else {
      state.containers = snapshot.containers
      state.metrics = snapshot.metrics
      recordAggregate(state, snapshot.metrics)
    }
    if ((snapshot.hostKey || LOCAL_HOST_KEY) === selectedHostKey.value) {
      loading.value = false
      error.value = null
    }
  }

  function applyInventorySnapshot(snapshot: DockerInventorySnapshot): void {
    const state = stateOf(snapshot.hostKey || LOCAL_HOST_KEY)
    state.volumes = snapshot.volumes
    state.networks = snapshot.networks
    state.images = snapshot.images
    if ((snapshot.hostKey || LOCAL_HOST_KEY) === selectedHostKey.value) {
      error.value = null
    }
  }

  /** `probe:status` de um agente: o host fica online/offline sem nova consulta. */
  function applyAgentStatus(data: Record<string, unknown>): void {
    const id = Number(data.id ?? data.probeId)
    const host = hosts.value.find((item) => item.agentId === id)
    if (!host) return
    host.online = data.status === 'online'
  }

  function applyOperationEvent(event: DockerOperationEvent): void {
    const existing = operations.value.find((item) => item.operationId === event.operationId)
    const updatedAt = new Date().toISOString()
    if (existing) {
      existing.state = event.state
      existing.message = event.message ?? existing.message
      existing.updatedAt = updatedAt
      if (event.line) existing.lines = [...existing.lines, event.line].slice(-OPERATION_LINES)
      return
    }
    operations.value = [
      {
        operationId: event.operationId,
        hostKey: event.hostKey,
        kind: event.kind,
        target: event.target,
        state: event.state,
        lines: event.line ? [event.line] : [],
        message: event.message,
        updatedAt,
      },
      ...operations.value,
    ].slice(0, OPERATIONS_KEPT)
  }

  function dismissOperation(operationId: string): void {
    operations.value = operations.value.filter((item) => item.operationId !== operationId)
  }

  function applyLogStreamEvent(event: DockerLogStreamEvent): void {
    const stream = logStreams.value[event.streamId]
    if (!stream) return
    if (event.entries.length > 0) {
      stream.entries = [...stream.entries, ...event.entries].slice(-LOG_LINES_KEPT)
    }
    if (event.ended) stream.ended = event.ended
  }

  /** Ação explícita do usuário: começar a acompanhar o log de um container. */
  async function startLogStream(containerId: string, tail: number | 'all' = 100): Promise<string> {
    const { streamId } = await api.value.followLogs(containerId, tail)
    logStreams.value = {
      ...logStreams.value,
      [streamId]: {
        streamId,
        hostKey: selectedHostKey.value,
        containerId,
        entries: [],
        ended: null,
      },
    }
    return streamId
  }

  async function stopLogStream(streamId: string): Promise<void> {
    logStreams.value = Object.fromEntries(
      Object.entries(logStreams.value).filter(([key]) => key !== streamId)
    )
    try {
      await dockerHostsService.stopLogStream(streamId)
    } catch {
      // O stream pode já ter terminado sozinho: nada a desfazer.
    }
  }

  /** Lista de hosts carregada ao abrir a tela; o estado segue pelo SSE. */
  async function fetchHosts(): Promise<void> {
    try {
      hosts.value = await dockerHostsService.hosts()
      if (!hosts.value.some((host) => host.key === selectedHostKey.value)) {
        selectedHostKey.value = LOCAL_HOST_KEY
      }
    } catch (reason: unknown) {
      fail(reason, 'Erro ao listar os hosts Docker')
    }
  }

  function selectHost(hostKey: string): void {
    selectedHostKey.value = hostKey || LOCAL_HOST_KEY
    stateOf(selectedHostKey.value)
    error.value = null
  }

  async function refreshAll(): Promise<void> {
    loading.value = true
    error.value = null
    const state = stateOf(selectedHostKey.value)
    const service = api.value
    try {
      state.status = await service.status()
      if (!state.status.available) {
        clearUnavailableState(state)
        return
      }

      const [containerResult, volumeResult, networkResult, imageResult, metricsResult] =
        await Promise.all([
          service.containers(),
          service.volumes(),
          service.networks(),
          service.images(),
          service.metrics(),
        ])
      state.containers = containerResult.data
      state.volumes = volumeResult.data
      state.networks = networkResult.data
      state.images = imageResult.data
      state.metrics = metricsResult
      recordAggregate(state, metricsResult)
    } catch (reason: unknown) {
      fail(reason, 'Erro ao consultar a Docker Engine')
    } finally {
      loading.value = false
    }
  }

  async function refreshContainers(): Promise<void> {
    const state = stateOf(selectedHostKey.value)
    const service = api.value
    try {
      const result = await service.containers()
      state.containers = result.data
      state.status = await service.status()
      state.metrics = await service.metrics()
      recordAggregate(state, state.metrics)
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
    hosts,
    hostStates,
    selectedHostKey,
    selectedHost,
    api,
    operations,
    logStreams,
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
    allows,
    hostView,
    applyLiveSnapshot,
    applyInventorySnapshot,
    applyAgentStatus,
    applyOperationEvent,
    dismissOperation,
    applyLogStreamEvent,
    startLogStream,
    stopLogStream,
    fetchHosts,
    selectHost,
    refreshAll,
    refreshContainers,
    runAction,
  }
})
