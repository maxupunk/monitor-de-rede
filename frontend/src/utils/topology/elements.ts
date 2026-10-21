/**
 * Mapeamento PURO do estado da topologia para elementos do Cytoscape.
 *
 * Tudo o que muda a aparência vira `data` (rótulos) ou `classes` (estado,
 * tipo, filtro, sobreposição); o estilo (`style.ts`) decide a cara de cada
 * classe. Assim o canvas só compara e aplica diferenças — sem regra de
 * negócio — e esta função é testável sem DOM.
 */
import type { TopologyEdge, TopologyNode, TopologyPosition } from '@/stores/topology'
import type { FilterResult } from './filters'
import {
  edgeEndpoints,
  edgeSpeed,
  edgeStatusKey,
  edgeTrafficLabel,
  edgeUtilization,
  isUnmanagedSwitch,
  linkKind,
  nodeIconKey,
  nodeHealthText,
  nodeIp,
  nodeStatusKey,
  speedTier,
  utilizationLevel,
} from './presentation'
import {
  EMPTY_OVERLAY,
  edgeOverlayClasses,
  highlightClass,
  nodeOverlayClasses,
  type OverlaySnapshot,
} from './overlay'

export type ElementData = Record<string, string | number | boolean | null>

export interface TopologyElement {
  group: 'nodes' | 'edges'
  data: ElementData & { id: string }
  classes: string
  position?: TopologyPosition
}

export interface ElementsInput {
  nodes: readonly TopologyNode[]
  edges: readonly TopologyEdge[]
  positions: Readonly<Record<number, TopologyPosition>>
  filter?: (FilterResult & { mode: 'hide' | 'dim' }) | null
  overlay?: OverlaySnapshot
  /** Desenha um nó composto por site e põe os dispositivos dentro. */
  groupBySite?: boolean
  /** Nomes de site para as caixas quando o nó não traz `siteName`. */
  siteNames?: Readonly<Record<number, string>>
  /** Linha de CPU/memória sob o nome (preferência "Exibir"). */
  showHealth?: boolean
  /** Linha de saúde já pronta por dispositivo (a do teste de velocidade), no lugar da do mapa. */
  healthLabels?: Readonly<Record<number, string>>
}

export const nodeElementId = (deviceId: number) => `n${deviceId}`
export const edgeElementId = (edgeId: number) => `e${edgeId}`
export const siteElementId = (siteId: number | null | undefined) => `s${siteId ?? 'none'}`

/** Id do Cytoscape → id do dispositivo/enlace (`null` se não for desse tipo). */
export function parseElementId(id: string): { kind: 'node' | 'edge' | 'site'; id: number } | null {
  const prefix = id.charAt(0)
  const value = Number(id.slice(1))
  if (prefix === 's') return { kind: 'site', id: Number.isFinite(value) ? value : -1 }
  if (!Number.isFinite(value)) return null
  if (prefix === 'n') return { kind: 'node', id: value }
  if (prefix === 'e') return { kind: 'edge', id: value }
  return null
}

function filterClass(
  filter: ElementsInput['filter'],
  matched: Set<number> | undefined,
  id: number
): string | null {
  if (!filter?.active || !matched || matched.has(id)) return null
  return filter.mode === 'hide' ? 'filtered-hidden' : 'filtered-dimmed'
}

export function nodeElement(
  node: TopologyNode,
  input: Pick<
    ElementsInput,
    'filter' | 'overlay' | 'groupBySite' | 'positions' | 'showHealth' | 'healthLabels'
  >
): TopologyElement {
  const overlay = input.overlay ?? EMPTY_OVERLAY
  const ip = nodeIp(node)
  const health = input.healthLabels?.[node.id] ?? (input.showHealth ? nodeHealthText(node) : null)
  const lines = [node.name, ip, health].filter((line): line is string => Boolean(line))
  const nodeOverlay = overlay.nodeOverlays[node.id]
  const classes = [
    `status-${nodeStatusKey(node.status)}`,
    node.isSystem ? 'system' : null,
    `rank-${nodeRank(node)}`,
    lines.length === 2 ? 'two-line' : null,
    lines.length === 3 ? 'three-line' : null,
    health ? 'has-health' : null,
    isUnmanagedSwitch(node) ? 'unmanaged' : null,
    filterClass(input.filter, input.filter?.nodes, node.id),
    highlightClass(overlay.highlight, 'node', node.id),
    ...nodeOverlayClasses(nodeOverlay),
  ].filter((value): value is string => Boolean(value))

  const data: TopologyElement['data'] = {
    id: nodeElementId(node.id),
    deviceId: node.id,
    // Os dois textos ficam fixos no dado; o nível de detalhe troca só a
    // CLASSE do nó (`lod-mid`/`lod-far`) e a folha de estilo escolhe qual
    // dado vira rótulo — trocar o texto de um rótulo já desenhado deixa o
    // Cytoscape com a caixa antiga (corta a 2ª linha).
    label: lines.join('\n'),
    name: node.name,
    // No nível médio (`lod-mid`) o IP sai, mas CPU/memória ficam: é a parte
    // que importa enquanto um teste enquadra o caminho.
    midLabel: health ? `${node.name}\n${health}` : node.name,
    iconKey: nodeIconKey(node),
  }
  if (input.groupBySite) data.parent = siteElementId(node.siteId)

  const element: TopologyElement = { group: 'nodes', data, classes: classes.join(' ') }
  const position = input.positions[node.id]
  if (position) element.position = { x: position.x, y: position.y }
  return element
}

/**
 * Peso do nó no nível de detalhe: `core` (servidor, roteador, firewall) tem o
 * nome visível até de muito longe; `infra` (switches, APs) de longe; `edge`
 * (a ponta) só de perto.
 */
export type NodeRank = 'core' | 'infra' | 'edge'

const CORE_ICONS = new Set(['system', 'router', 'firewall'])
const INFRA_ICONS = new Set(['switch', 'unmanaged_switch', 'ap'])

export function nodeRank(node: Pick<TopologyNode, 'type' | 'isSystem'>): NodeRank {
  const key = nodeIconKey(node)
  if (CORE_ICONS.has(key)) return 'core'
  return INFRA_ICONS.has(key) ? 'infra' : 'edge'
}

export function edgeElement(
  edge: TopologyEdge,
  input: Pick<ElementsInput, 'filter' | 'overlay'>
): TopologyElement {
  const overlay = input.overlay ?? EMPTY_OVERLAY
  const { source, target } = edgeEndpoints(edge)
  const edgeOverlay = overlay.edgeOverlays[edge.id]
  const level = utilizationLevel(edgeUtilization(edge))
  const kind = linkKind(edge.linkType)
  const classes = [
    `kind-${kind}`,
    `status-${edgeStatusKey(edge)}`,
    `tier-${speedTier(edgeSpeed(edge))}`,
    level ? `util-${level}` : null,
    edge.stale ? 'stale' : null,
    filterClass(input.filter, input.filter?.edges, edge.id),
    highlightClass(overlay.highlight, 'edge', edge.id),
    ...edgeOverlayClasses(edgeOverlay),
  ].filter((value): value is string => Boolean(value))

  return {
    group: 'edges',
    data: {
      id: edgeElementId(edge.id),
      edgeId: edge.id,
      source: nodeElementId(source),
      target: nodeElementId(target),
      trafficLabel: edgeTrafficLabel(edge) ?? '',
      overlayLabel: edgeOverlay?.label ?? '',
    },
    classes: classes.join(' '),
  }
}

export function buildElements(input: ElementsInput): TopologyElement[] {
  const elements: TopologyElement[] = []
  const nodeIds = new Set<number>()

  if (input.groupBySite) {
    const sites = new Map<string, string>()
    for (const node of input.nodes) {
      const id = siteElementId(node.siteId)
      if (!sites.has(id)) {
        const named = node.siteId ? input.siteNames?.[node.siteId] : undefined
        sites.set(id, node.siteName || named || (node.siteId ? `Site #${node.siteId}` : 'Sem site'))
      }
    }
    for (const [id, label] of sites) {
      elements.push({ group: 'nodes', data: { id, label }, classes: 'site-group' })
    }
  }

  for (const node of input.nodes) {
    nodeIds.add(node.id)
    elements.push(nodeElement(node, input))
  }

  for (const edge of input.edges) {
    const { source, target } = edgeEndpoints(edge)
    // Aresta para fora do recorte (ou laço) não tem como ser desenhada.
    if (!nodeIds.has(source) || !nodeIds.has(target) || source === target) continue
    elements.push(edgeElement(edge, input))
  }
  return elements
}
