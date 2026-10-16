import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

/**
 * Camada visual sobre o mapa de topologia, desenhada por quem precisa contar
 * algo no mapa sem mexer no grafo — o teste de enlace (ADR 015) é o primeiro
 * cliente: destaca o caminho, anima o enlace em teste e pendura o resultado.
 *
 * Só estado: o `TopologyCanvas` traduz isto em classes do Cytoscape
 * (`utils/topology/overlay.ts`).
 */

export type OverlayState = 'pending' | 'running' | 'succeeded' | 'failed' | 'skipped'
export type OverlayFlow = 'forward' | 'reverse' | 'both'

export interface EdgeOverlay {
  state: OverlayState
  /** Texto curto na pílula do enlace (ex.: "↓ 487 Mbps ↑ 471 Mbps"). */
  label?: string
  /** Sentido da animação enquanto `running` (origem → destino = `forward`). */
  flow?: OverlayFlow
}

export interface NodeOverlay {
  state: OverlayState
  label?: string
}

export interface PathHighlight {
  nodeIds: number[]
  edgeIds: number[]
}

export const useTopologyOverlayStore = defineStore('topologyOverlay', () => {
  /** Destaque de caminho: o resto do mapa esmaece. */
  const highlight = ref<PathHighlight | null>(null)
  const edgeOverlays = ref<Record<number, EdgeOverlay>>({})
  const nodeOverlays = ref<Record<number, NodeOverlay>>({})

  const hasRunning = computed(() =>
    Object.values(edgeOverlays.value).some((overlay) => overlay.state === 'running')
  )
  const isActive = computed(
    () =>
      highlight.value !== null ||
      Object.keys(edgeOverlays.value).length > 0 ||
      Object.keys(nodeOverlays.value).length > 0
  )

  function setPathHighlight(path: PathHighlight | null) {
    highlight.value = path
      ? { nodeIds: [...new Set(path.nodeIds)], edgeIds: [...new Set(path.edgeIds)] }
      : null
  }

  function setEdgeOverlay(edgeId: number, overlay: EdgeOverlay | null) {
    const next = { ...edgeOverlays.value }
    if (overlay) next[edgeId] = { ...overlay }
    else delete next[edgeId]
    edgeOverlays.value = next
  }

  function setNodeOverlay(deviceId: number, overlay: NodeOverlay | null) {
    const next = { ...nodeOverlays.value }
    if (overlay) next[deviceId] = { ...overlay }
    else delete next[deviceId]
    nodeOverlays.value = next
  }

  function clear() {
    highlight.value = null
    edgeOverlays.value = {}
    nodeOverlays.value = {}
  }

  return {
    highlight,
    edgeOverlays,
    nodeOverlays,
    hasRunning,
    isActive,
    setPathHighlight,
    setEdgeOverlay,
    setNodeOverlay,
    clear,
  }
})
