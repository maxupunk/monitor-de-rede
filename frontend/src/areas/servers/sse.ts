import type { SseHandler, SseRoutes } from '@/stores/events'
import { useDockerStore } from '@/stores/docker'
import { isDeployLines, isDeployRun, useDeploysStore } from '@/stores/deploys'

/** Cada evento `docker:*` vai para a ação da store Docker que o aplica. */
const DOCKER_EVENTS: Record<string, SseHandler> = {
  'docker:snapshot': (data) => useDockerStore().applyLiveSnapshot(data as never),
  'docker:inventory': (data) => useDockerStore().applyInventorySnapshot(data as never),
  'docker:operation': (data) => useDockerStore().applyOperationEvent(data as never),
  'docker:log': (data) => useDockerStore().applyLogStreamEvent(data as never),
}

/** Servidores: o Docker dos agentes. Tudo `docker:*` é estado ao vivo, fora do feed. */
export const serversSse: SseRoutes = {
  'docker:*': {
    ephemeral: true,
    handle: (data, payload, bus) => DOCKER_EVENTS[payload.type]?.(data, payload, bus),
  },
  // Deploy contínuo (ADR 021): o estado de cada deploy e as linhas do console
  // são estado ao vivo; o cadastro mudou, a página relê a visão geral uma vez.
  'deploys:run': {
    ephemeral: true,
    handle: (data) => {
      if (isDeployRun(data)) useDeploysStore().applyRun(data)
    },
  },
  'deploys:log': {
    ephemeral: true,
    handle: (data) => {
      if (isDeployLines(data)) useDeploysStore().applyLines(data.runId, data.lines)
    },
  },
  'deploys:updated': {
    ephemeral: true,
    handle: (_data, _payload, bus) =>
      bus.scheduleRefresh('deploys', () => useDeploysStore().refreshIfLoaded()),
  },
  // O seletor de host Docker acompanha o estado dos agentes (ADR 011).
  'probe:status': (data) => useDockerStore().applyAgentStatus(data),
}
