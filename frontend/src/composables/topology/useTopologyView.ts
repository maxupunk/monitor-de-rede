import { computed, ref } from 'vue'
import { useSitesStore } from '@/stores/sites'
import { useTopologyStore } from '@/stores/topology'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import { buildElements } from '@/utils/topology/elements'
import { applyFilters, isProblemEdge } from '@/utils/topology/filters'
import type { TabChoice, TopologyTarget } from '@/utils/topology/inspectorRegistry'
import { edgeStatusKey, nodeStatusKey } from '@/utils/topology/presentation'
import type { TopologyPrefs } from '@/utils/topology/prefs'
import type { CanvasPointerEvent } from '@/utils/topology/canvasTypes'
import type { Selection } from './useTopologyHost'

/**
 * Estado derivado do mapa: filtro, elementos do canvas, resumo, seleção e
 * hover — tudo computado do store, sem regra de desenho.
 */
export function useTopologyView(prefs: TopologyPrefs) {
  const store = useTopologyStore()
  const overlay = useTopologyOverlayStore()
  const sites = useSitesStore()

  const selection = ref<Selection | null>(null)
  const hover = ref<CanvasPointerEvent | null>(null)
  const inspectorTab = ref<TabChoice>({})

  /** Abre a aba no inspetor do item selecionado (ou nos dois tipos, sem seleção). */
  function openTab(tabId: string) {
    const kind = selection.value?.kind
    inspectorTab.value = kind
      ? { ...inspectorTab.value, [kind]: tabId }
      : { node: tabId, edge: tabId }
  }

  const filterResult = computed(() => applyFilters(store.nodes, store.edges, prefs.filters))

  const siteNames = computed(() =>
    Object.fromEntries(sites.sites.map((site) => [site.id, site.name] as const))
  )

  const elements = computed(() =>
    buildElements({
      nodes: store.nodes,
      edges: store.edges,
      positions: store.layout,
      filter: { ...filterResult.value, mode: prefs.filters.mode },
      overlay: {
        highlight: overlay.highlight,
        edgeOverlays: overlay.edgeOverlays,
        nodeOverlays: overlay.nodeOverlays,
      },
      groupBySite: prefs.groupBySite,
      siteNames: siteNames.value,
      showHealth: prefs.showHealth,
    })
  )

  const summary = computed(() => ({
    nodes: store.nodes.length,
    edges: store.edges.length,
    offline: store.nodes.filter((node) => nodeStatusKey(node.status) === 'offline').length,
    warning: store.nodes.filter((node) => nodeStatusKey(node.status) === 'warning').length,
    linksDown: store.edges.filter((edge) => edgeStatusKey(edge) === 'down').length,
    saturated: store.edges.filter((edge) => edgeStatusKey(edge) !== 'down' && isProblemEdge(edge))
      .length,
  }))

  /** O alvo selecionado com o objeto VIVO do store (SSE atualiza o painel). */
  const selectedTarget = computed<TopologyTarget | null>(() => {
    const current = selection.value
    if (!current) return null
    if (current.kind === 'node') {
      const node = store.nodesById.get(current.id)
      return node ? { kind: 'node', node } : null
    }
    const edge = store.edgesById.get(current.id)
    return edge ? { kind: 'edge', edge } : null
  })

  const hoveredNode = computed(() =>
    hover.value?.kind === 'node' && hover.value.id !== undefined
      ? (store.nodesById.get(hover.value.id) ?? null)
      : null
  )
  const hoveredEdge = computed(() =>
    hover.value?.kind === 'edge' && hover.value.id !== undefined
      ? (store.edgesById.get(hover.value.id) ?? null)
      : null
  )

  function targetFor(event: CanvasPointerEvent): TopologyTarget | null {
    if (event.kind === 'background') return { kind: 'background' }
    if (event.kind === 'node' && event.id !== undefined) {
      const node = store.nodesById.get(event.id)
      return node ? { kind: 'node', node } : null
    }
    if (event.kind === 'edge' && event.id !== undefined) {
      const edge = store.edgesById.get(event.id)
      return edge ? { kind: 'edge', edge } : null
    }
    return null
  }

  return {
    selection,
    hover,
    inspectorTab,
    openTab,
    filterResult,
    elements,
    summary,
    selectedTarget,
    hoveredNode,
    hoveredEdge,
    targetFor,
  }
}
