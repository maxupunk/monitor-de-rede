import type { SseHandler, SseRoutes } from '@/stores/events'
import { useDockerStore } from '@/stores/docker'
import { isDeployLines, isDeployRun, useDeploysStore } from '@/stores/deploys'
import { isBuildLines, isServerBuild, useDeployBuildsStore } from '@/stores/deployBuilds'
import { useRegistryStore } from '@/stores/registry'

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
  // Compilação no servidor (ADR 023): o estado e o console, ao vivo.
  'deploys:build': {
    ephemeral: true,
    handle: (data) => {
      if (isServerBuild(data)) useDeployBuildsStore().apply(data)
    },
  },
  'deploys:build-log': {
    ephemeral: true,
    handle: (data) => {
      if (isBuildLines(data)) useDeployBuildsStore().applyLines(data.buildId, data.lines)
    },
  },
  // O catálogo do registry mudou (imagem nova, análise, limpeza).
  'registry:changed': {
    ephemeral: true,
    handle: (_data, _payload, bus) =>
      bus.scheduleRefresh('registry', () => useRegistryStore().refreshIfLoaded()),
  },
  'deploys:updated': {
    ephemeral: true,
    handle: (_data, _payload, bus) =>
      bus.scheduleRefresh('deploys', () => useDeploysStore().refreshIfLoaded()),
  },
  // O seletor de host Docker acompanha o estado dos agentes (ADR 011); o
  // guia do deploy também (a etapa "servidor com o agente" anda sozinha).
  'probe:status': (data, _payload, bus) => {
    useDockerStore().applyAgentStatus(data)
    bus.scheduleRefresh('deploys', () => useDeploysStore().refreshIfLoaded())
  },
}
