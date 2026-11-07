import type { SseHandler, SseRoutes } from '@/stores/events'
import { useDockerStore } from '@/stores/docker'

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
  // O seletor de host Docker acompanha o estado dos agentes (ADR 011).
  'probe:status': (data) => useDockerStore().applyAgentStatus(data),
}
