/**
 * Filtros do mapa — funções puras sobre o grafo. O site é recorte do backend
 * (`?siteId=`); o resto é decidido aqui e vira classe no canvas
 * (`hidden`/`dimmed`, conforme o modo escolhido).
 */
import type { TopologyEdge, TopologyNode } from '@/stores/topology'
import {
  edgeEndpoints,
  edgeStatusKey,
  edgeUtilization,
  linkKind,
  nodeCategory,
  nodeStatusKey,
  utilizationLevel,
  type LinkKind,
  type NodeStatusKey,
} from './presentation'

export interface TopologyFilters {
  statuses: NodeStatusKey[]
  categories: string[]
  linkKinds: LinkKind[]
  /** Só o que precisa de atenção: offline/alerta, enlace fora ou saturado. */
  problemsOnly: boolean
  /** O que não casa some (`hide`) ou fica apagado (`dim`). */
  mode: 'hide' | 'dim'
}

export const EMPTY_FILTERS: TopologyFilters = {
  statuses: [],
  categories: [],
  linkKinds: [],
  problemsOnly: false,
  mode: 'dim',
}

export function activeFilterCount(filters: TopologyFilters): number {
  return (
    filters.statuses.length +
    filters.categories.length +
    filters.linkKinds.length +
    (filters.problemsOnly ? 1 : 0)
  )
}

export function isProblemEdge(edge: TopologyEdge): boolean {
  if (edgeStatusKey(edge) === 'down') return true
  const level = utilizationLevel(edgeUtilization(edge))
  return level === 'high' || level === 'critical'
}

export function isProblemNode(node: TopologyNode): boolean {
  const status = nodeStatusKey(node.status)
  return status === 'offline' || status === 'warning'
}

export interface FilterResult {
  /** Ids de nós que casam com o filtro. */
  nodes: Set<number>
  /** Ids de arestas que casam com o filtro. */
  edges: Set<number>
  /** `true` quando há algum filtro ativo. */
  active: boolean
}

/**
 * Nó casa se passa por status, categoria e "só problemas" (um nó com enlace
 * problemático conta como problema). Aresta casa se o tipo passa e as duas
 * pontas casam — ou, com "só problemas", se ela mesma é o problema.
 */
export function applyFilters(
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[],
  filters: TopologyFilters
): FilterResult {
  const active = activeFilterCount(filters) > 0
  if (!active) {
    return {
      nodes: new Set(nodes.map((node) => node.id)),
      edges: new Set(edges.map((edge) => edge.id)),
      active,
    }
  }

  const problemEdges = new Set<number>()
  const nodesWithProblemEdge = new Set<number>()
  if (filters.problemsOnly) {
    for (const edge of edges) {
      if (!isProblemEdge(edge)) continue
      problemEdges.add(edge.id)
      const { source, target } = edgeEndpoints(edge)
      nodesWithProblemEdge.add(source)
      nodesWithProblemEdge.add(target)
    }
  }

  const statuses = new Set(filters.statuses)
  const categories = new Set(filters.categories)
  const matchedNodes = new Set<number>()
  for (const node of nodes) {
    if (statuses.size > 0 && !statuses.has(nodeStatusKey(node.status))) continue
    if (categories.size > 0 && !categories.has(nodeCategory(node))) continue
    if (filters.problemsOnly && !isProblemNode(node) && !nodesWithProblemEdge.has(node.id)) continue
    matchedNodes.add(node.id)
  }

  const kinds = new Set(filters.linkKinds)
  const matchedEdges = new Set<number>()
  for (const edge of edges) {
    if (kinds.size > 0 && !kinds.has(linkKind(edge.linkType))) continue
    const { source, target } = edgeEndpoints(edge)
    const endpointsMatch = matchedNodes.has(source) && matchedNodes.has(target)
    if (filters.problemsOnly ? problemEdges.has(edge.id) || endpointsMatch : endpointsMatch) {
      matchedEdges.add(edge.id)
    }
  }

  return { nodes: matchedNodes, edges: matchedEdges, active }
}
