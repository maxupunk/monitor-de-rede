/**
 * Layouts automáticos do mapa.
 *
 * - Os pesados (hierárquico, radial, orgânico, por site) rodam o ELK num Web
 *   Worker (`elk.worker.ts`) — a tela não trava com 800 nós.
 * - Grade e "organizar só os novos" são funções puras aqui mesmo.
 *
 * Regra de ouro: nenhum layout move um nó que o operador posicionou, a não
 * ser quando ele pede "Reorganizar tudo" (com confirmação).
 */
import type { TopologyEdge, TopologyNode, TopologyPosition } from '@/stores/topology'
import { edgeEndpoints, nodeCategory } from './presentation'
import {
  arrangeCompact,
  arrangeSubtree,
  childrenPack,
  closedNodes,
  compareNodes,
  isolatedGrid,
  leafGrid,
  spanningForest,
  LEAF_GROUP_MIN,
  LEVEL_GAP,
  NODE_BOX,
  type TreeBox,
} from './treeLayout'

export type LayoutKind = 'hierarchical' | 'radial' | 'organic' | 'grid' | 'sites'

export const LAYOUT_OPTIONS: ReadonlyArray<{
  value: LayoutKind
  title: string
  subtitle: string
  icon: string
}> = [
  {
    value: 'hierarchical',
    title: 'Hierárquico',
    subtitle: 'Do roteador de borda até a ponta, em árvore compacta',
    icon: 'mdi-family-tree',
  },
  {
    value: 'radial',
    title: 'Árvore radial',
    subtitle: 'Anéis a partir do núcleo da rede',
    icon: 'mdi-chart-donut-variant',
  },
  {
    value: 'organic',
    title: 'Orgânico',
    subtitle: 'Agrupa naturalmente quem está ligado',
    icon: 'mdi-graph-outline',
  },
  {
    value: 'grid',
    title: 'Grade',
    subtitle: 'Linhas organizadas por tipo de dispositivo',
    icon: 'mdi-view-grid-outline',
  },
  {
    value: 'sites',
    title: 'Agrupar por site',
    subtitle: 'Uma caixa por site, hierárquico dentro',
    icon: 'mdi-office-building-marker-outline',
  },
]

export type PositionMap = Record<number, TopologyPosition>

export { LEAF_GROUP_MIN, NODE_BOX } from './treeLayout'

// --- Raízes ------------------------------------------------------------------

const ROOT_CATEGORIES = new Set(['router', 'firewall'])

/** Distâncias (em saltos) a partir de um nó. */
function hops(start: number, adjacency: ReadonlyMap<number, readonly number[]>) {
  const distance = new Map<number, number>([[start, 0]])
  const queue = [start]
  while (queue.length > 0) {
    const current = queue.shift()!
    for (const other of adjacency.get(current) ?? []) {
      if (distance.has(other)) continue
      distance.set(other, distance.get(current)! + 1)
      queue.push(other)
    }
  }
  return distance
}

/**
 * Raiz de cada componente ligado (nós sem enlace não precisam de raiz): o
 * roteador de borda — o roteador (senão o firewall) mais perto do servidor
 * NetMonitor, que é por onde a rede sai; sem roteador, o servidor; sem
 * servidor, o nó de maior grau. A internet fica em cima, a ponta embaixo.
 */
export function chooseRoots(nodes: readonly TopologyNode[], edges: readonly TopologyEdge[]) {
  const byId = new Map(nodes.map((node) => [node.id, node]))
  const adjacency = new Map<number, number[]>()
  for (const edge of edges) {
    const { source, target } = edgeEndpoints(edge)
    if (source === target || !byId.has(source) || !byId.has(target)) continue
    if (!adjacency.has(source)) adjacency.set(source, [])
    if (!adjacency.has(target)) adjacency.set(target, [])
    adjacency.get(source)!.push(target)
    adjacency.get(target)!.push(source)
  }
  const degree = (id: number) => adjacency.get(id)?.length ?? 0
  const byDegree = (a: number, b: number) => degree(b) - degree(a) || a - b

  const roots: number[] = []
  const seen = new Set<number>()
  for (const node of [...nodes].sort(compareNodes)) {
    if (seen.has(node.id) || !adjacency.has(node.id)) continue
    const reach = hops(node.id, adjacency)
    const members = [...reach.keys()]
    members.forEach((id) => seen.add(id))
    const system = members.find((id) => byId.get(id)?.isSystem)
    const routers = members.filter((id) => nodeCategory(byId.get(id)!) === 'router')
    const firewalls = members.filter((id) => nodeCategory(byId.get(id)!) === 'firewall')
    const candidates = routers.length > 0 ? routers : firewalls
    if (candidates.length > 0) {
      const fromSystem = system !== undefined ? hops(system, adjacency) : null
      const distance = (id: number) => fromSystem?.get(id) ?? 0
      roots.push([...candidates].sort((a, b) => distance(a) - distance(b) || byDegree(a, b))[0])
    } else if (system !== undefined) {
      roots.push(system)
    } else {
      roots.push([...members].sort(byDegree)[0])
    }
  }
  return roots
}

/**
 * Orienta cada enlace a partir das raízes (busca em largura): no layout em
 * camadas, a seta "pai → filho" é o que põe o núcleo em cima.
 */
export function orientEdges(
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[],
  roots: readonly number[]
): Array<{ id: number; source: number; target: number }> {
  const adjacency = new Map<number, Array<{ id: number; other: number }>>()
  const add = (from: number, to: number, id: number) => {
    if (!adjacency.has(from)) adjacency.set(from, [])
    adjacency.get(from)!.push({ id, other: to })
  }
  const known = new Set(nodes.map((node) => node.id))
  for (const edge of edges) {
    const { source, target } = edgeEndpoints(edge)
    if (!known.has(source) || !known.has(target) || source === target) continue
    add(source, target, edge.id)
    add(target, source, edge.id)
  }

  const depth = new Map<number, number>()
  const queue: number[] = []
  for (const root of roots) {
    if (known.has(root) && !depth.has(root)) {
      depth.set(root, 0)
      queue.push(root)
    }
  }
  // Componentes sem raiz: começam pelo primeiro nó que aparecer.
  const visit = () => {
    while (queue.length > 0) {
      const current = queue.shift()!
      for (const { other } of adjacency.get(current) ?? []) {
        if (depth.has(other)) continue
        depth.set(other, (depth.get(current) ?? 0) + 1)
        queue.push(other)
      }
    }
  }
  visit()
  for (const node of nodes) {
    if (depth.has(node.id)) continue
    depth.set(node.id, 0)
    queue.push(node.id)
    visit()
  }

  const result: Array<{ id: number; source: number; target: number }> = []
  for (const edge of edges) {
    const { source, target } = edgeEndpoints(edge)
    if (!known.has(source) || !known.has(target) || source === target) continue
    const flip = (depth.get(source) ?? 0) > (depth.get(target) ?? 0)
    result.push({ id: edge.id, source: flip ? target : source, target: flip ? source : target })
  }
  return result
}

// --- Grafo ELK ----------------------------------------------------------------

export interface ElkNodeJson {
  id: string
  width?: number
  height?: number
  x?: number
  y?: number
  layoutOptions?: Record<string, string>
  children?: ElkNodeJson[]
  edges?: ElkEdgeJson[]
}

export interface ElkEdgeJson {
  id: string
  sources: string[]
  targets: string[]
}

/** Blocos já arrumados que o ELK trata como uma caixa só (id ELK → bloco). */
export type ElkBlocks = ReadonlyMap<string, TreeBox>

/** O que vai para o worker e como ler a volta. */
export interface ElkPlan {
  graph: ElkNodeJson
  blocks: ElkBlocks
}

const BASE_OPTIONS: Record<string, string> = {
  'elk.spacing.nodeNode': '40',
  'elk.padding': '[top=40,left=40,bottom=40,right=40]',
}

const OPTIONS: Record<Exclude<LayoutKind, 'grid'>, Record<string, string>> = {
  hierarchical: {
    'elk.algorithm': 'layered',
    'elk.direction': 'DOWN',
    'elk.layered.spacing.nodeNodeBetweenLayers': String(LEVEL_GAP),
    'elk.spacing.nodeNode': '32',
    'elk.layered.crossingMinimization.strategy': 'LAYER_SWEEP',
    'elk.layered.crossingMinimization.greedySwitch.type': 'TWO_SIDED',
    'elk.layered.nodePlacement.strategy': 'BRANDES_KOEPF',
    'elk.layered.nodePlacement.bk.fixedAlignment': 'BALANCED',
    'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    'elk.separateConnectedComponents': 'true',
    'elk.spacing.componentComponent': '120',
    'elk.aspectRatio': '1.6',
  },
  radial: {
    'elk.algorithm': 'radial',
    'elk.radial.compactor': 'WEDGE_COMPACTION',
    'elk.spacing.nodeNode': '30',
  },
  organic: {
    'elk.algorithm': 'stress',
    'elk.stress.desiredEdgeLength': '170',
    'elk.spacing.nodeNode': '50',
  },
  sites: {
    'elk.algorithm': 'layered',
    'elk.direction': 'DOWN',
    'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
    'elk.layered.spacing.nodeNodeBetweenLayers': '80',
    'elk.spacing.nodeNode': '30',
  },
}

/** Folhas penduradas no mesmo vizinho (PCs num switch de acesso). */
export interface LeafGroup {
  anchor: number
  leaves: number[]
  box: TreeBox
}

/**
 * Agrupa as folhas (grau 1) por vizinho. No layout, cada grupo vira um bloco
 * compacto embaixo do vizinho — sem isso, 40 PCs num switch viram uma fileira
 * de 6000 px e o mapa inteiro fica minúsculo.
 */
export function groupLeaves(
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[]
): Map<number, LeafGroup> {
  const byId = new Map(nodes.map((node) => [node.id, node]))
  const neighbors = new Map<number, Set<number>>()
  for (const edge of edges) {
    const { source, target } = edgeEndpoints(edge)
    if (source === target || !byId.has(source) || !byId.has(target)) continue
    if (!neighbors.has(source)) neighbors.set(source, new Set())
    if (!neighbors.has(target)) neighbors.set(target, new Set())
    neighbors.get(source)!.add(target)
    neighbors.get(target)!.add(source)
  }
  const pending = new Map<number, TopologyNode[]>()
  for (const node of nodes) {
    const around = neighbors.get(node.id)
    // Roteador/firewall na ponta continua nó próprio: é núcleo, não "folha".
    if (!around || around.size !== 1 || node.isSystem || ROOT_CATEGORIES.has(nodeCategory(node)))
      continue
    const anchor = [...around][0]
    // Folha que também é a única ligação do vizinho: é um par, não um bloco.
    if ((neighbors.get(anchor)?.size ?? 0) <= 1) continue
    if ((byId.get(anchor)?.siteId ?? null) !== (node.siteId ?? null)) continue
    if (!pending.has(anchor)) pending.set(anchor, [])
    pending.get(anchor)!.push(node)
  }
  const groups = new Map<number, LeafGroup>()
  for (const [anchor, leaves] of pending) {
    if (leaves.length < LEAF_GROUP_MIN) continue
    const ids = [...leaves].sort(compareNodes).map((node) => node.id)
    groups.set(anchor, { anchor, leaves: ids, box: leafGrid(ids) })
  }
  return groups
}

const elkNodeId = (id: number) => `n${id}`
const elkGroupId = (anchor: number) => `g${anchor}`
const elkSiteId = (siteId: number | null | undefined) => `s${siteId ?? 'none'}`

/** Os blocos de folhas de `groupLeaves`, no formato que `positionsFromElk` abre. */
export function leafGroupBlocks(groups: ReadonlyMap<number, LeafGroup>): ElkBlocks {
  return new Map([...groups.values()].map((group) => [elkGroupId(group.anchor), group.box]))
}

/** Monta o grafo que o ELK posiciona (radial, orgânico, por site). Pura — o worker só executa. */
export function buildElkGraph(
  kind: Exclude<LayoutKind, 'grid'>,
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[],
  groups: ReadonlyMap<number, LeafGroup> = new Map()
): ElkNodeJson {
  const grouped = new Set<number>()
  for (const group of groups.values()) for (const id of group.leaves) grouped.add(id)
  const visible = nodes.filter((node) => !grouped.has(node.id))
  const visibleEdges = edges.filter((edge) => {
    const { source, target } = edgeEndpoints(edge)
    return !grouped.has(source) && !grouped.has(target)
  })

  const roots = chooseRoots(visible, visibleEdges)
  const oriented = orientEdges(visible, visibleEdges, roots)
  let elkEdges: ElkEdgeJson[] = oriented.map((edge) => ({
    id: `e${edge.id}`,
    sources: [elkNodeId(edge.source)],
    targets: [elkNodeId(edge.target)],
  }))
  for (const group of groups.values()) {
    elkEdges.push({
      id: `ge${group.anchor}`,
      sources: [elkNodeId(group.anchor)],
      targets: [elkGroupId(group.anchor)],
    })
  }
  if (kind === 'radial') {
    // Radial exige árvore: fica só a primeira aresta que chega em cada nó.
    const seen = new Set<string>()
    elkEdges = elkEdges.filter((edge) => {
      const target = edge.targets[0]
      if (seen.has(target)) return false
      seen.add(target)
      return true
    })
  }

  const single = (node: TopologyNode): ElkNodeJson => ({
    id: elkNodeId(node.id),
    width: NODE_BOX.width,
    height: NODE_BOX.height,
  })
  const block = (group: LeafGroup): ElkNodeJson => ({
    id: elkGroupId(group.anchor),
    width: group.box.width,
    height: group.box.height,
  })

  const root: ElkNodeJson = {
    id: 'root',
    layoutOptions: { ...BASE_OPTIONS, ...OPTIONS[kind] },
    edges: elkEdges,
  }

  if (kind === 'sites') {
    const sites = new Map<string, ElkNodeJson>()
    const siteOf = new Map(nodes.map((node) => [node.id, elkSiteId(node.siteId)]))
    const box = (id: string): ElkNodeJson => {
      if (!sites.has(id)) {
        sites.set(id, {
          id,
          layoutOptions: { 'elk.padding': '[top=60,left=40,bottom=40,right=40]' },
          children: [],
        })
      }
      return sites.get(id)!
    }
    for (const node of visible) box(siteOf.get(node.id)!).children!.push(single(node))
    for (const group of groups.values()) {
      box(siteOf.get(group.anchor) ?? elkSiteId(null)).children!.push(block(group))
    }
    root.children = [...sites.values()]
  } else {
    // O radial precisa da raiz como primeiro filho.
    const ordered =
      kind === 'radial' && roots.length > 0
        ? [...visible].sort((a, b) => Number(b.id === roots[0]) - Number(a.id === roots[0]))
        : visible
    root.children = [...ordered.map(single), ...[...groups.values()].map(block)]
  }
  return root
}

/**
 * Hierárquico: a parte que é árvore pura vira bloco compacto (`treeLayout`);
 * só o núcleo com enlaces redundantes vai como nós soltos para o ELK
 * minimizar cruzamentos. Cada nó do núcleo leva junto o conjunto dos filhos
 * que são árvore (`p<id>`); componentes que são árvore inteira viram um bloco
 * (`b<id>`); nós sem enlace, uma grade (`i0`).
 */
export function buildHierarchy(
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[]
): ElkPlan {
  const roots = chooseRoots(nodes, edges)
  const forest = spanningForest(nodes, edges, roots)
  const closed = closedNodes(forest)
  const blocks = new Map<string, TreeBox>()
  const children: ElkNodeJson[] = []
  const box = (id: string, tree: TreeBox): ElkNodeJson => {
    blocks.set(id, tree)
    return { id, width: tree.width, height: tree.height }
  }

  const core = nodes.filter((node) => forest.parent.has(node.id) && !closed.has(node.id))
  const coreIds = new Set(core.map((node) => node.id))
  const packEdges: ElkEdgeJson[] = []
  for (const node of core) {
    children.push({ id: elkNodeId(node.id), width: NODE_BOX.width, height: NODE_BOX.height })
    const kids = (forest.children.get(node.id) ?? []).filter((id) => closed.has(id))
    const pack = arrangeCompact((options) => childrenPack(kids, forest.children, options))
    if (!pack) continue
    children.push(box(`p${node.id}`, pack))
    packEdges.push({ id: `pe${node.id}`, sources: [elkNodeId(node.id)], targets: [`p${node.id}`] })
  }
  for (const [id, parent] of forest.parent) {
    if (parent !== null || !closed.has(id)) continue
    const tree = arrangeCompact((options) => arrangeSubtree(id, forest.children, options))
    if (tree) children.push(box(`b${id}`, tree))
  }
  const isolated = isolatedGrid(forest.isolated)
  if (isolated) children.push(box('i0', isolated))

  const coreEdges = edges.filter((edge) => {
    const { source, target } = edgeEndpoints(edge)
    return coreIds.has(source) && coreIds.has(target)
  })
  const oriented = orientEdges(core, coreEdges, roots).map((edge) => ({
    id: `e${edge.id}`,
    sources: [elkNodeId(edge.source)],
    targets: [elkNodeId(edge.target)],
  }))

  return {
    graph: {
      id: 'root',
      layoutOptions: { ...BASE_OPTIONS, ...OPTIONS.hierarchical },
      children,
      edges: [...oriented, ...packEdges],
    },
    blocks,
  }
}

/** Posições (centro do nó) a partir do resultado do ELK, abrindo os blocos. */
export function positionsFromElk(graph: ElkNodeJson, blocks: ElkBlocks = new Map()): PositionMap {
  const positions: PositionMap = {}
  const walk = (node: ElkNodeJson, offsetX: number, offsetY: number) => {
    const x = offsetX + (node.x ?? 0)
    const y = offsetY + (node.y ?? 0)
    const block = blocks.get(node.id)
    const id = Number(node.id.slice(1))
    if (block) {
      for (const [leaf, pos] of block.positions) {
        positions[leaf] = { x: Math.round(x + pos.x), y: Math.round(y + pos.y) }
      }
    } else if (node.id.startsWith('n') && Number.isFinite(id)) {
      positions[id] = {
        x: Math.round(x + (node.width ?? NODE_BOX.width) / 2),
        y: Math.round(y + (node.height ?? NODE_BOX.height) / 2),
      }
    }
    for (const child of node.children ?? []) walk(child, x, y)
  }
  for (const child of graph.children ?? []) walk(child, 0, 0)
  return positions
}

// --- Grade --------------------------------------------------------------------

/** Grade com uma faixa por tipo, do núcleo para a borda. */
export function gridLayout(nodes: readonly TopologyNode[], columns?: number): PositionMap {
  const cols = columns ?? Math.max(4, Math.ceil(Math.sqrt(nodes.length * 1.6)))
  const sorted = [...nodes].sort(compareNodes)
  const positions: PositionMap = {}
  sorted.forEach((node, index) => {
    positions[node.id] = {
      x: (index % cols) * NODE_BOX.width + NODE_BOX.width / 2,
      y: Math.floor(index / cols) * (NODE_BOX.height + 30) + NODE_BOX.height / 2,
    }
  })
  return positions
}

// --- Organizar só os novos ------------------------------------------------------

const SLOT_DISTANCE = 130
const MIN_GAP = 95

function isFree(candidate: TopologyPosition, taken: readonly TopologyPosition[]): boolean {
  return taken.every((pos) => Math.hypot(pos.x - candidate.x, pos.y - candidate.y) >= MIN_GAP)
}

/**
 * Posiciona SÓ os nós sem posição, sem tocar nos outros: perto dos vizinhos
 * já posicionados (em volta deles, numa vaga livre), ou numa fileira abaixo
 * do mapa quando não têm vizinho posicionado. Devolve apenas os novos.
 */
export function placeNewNodes(
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[],
  positions: Readonly<PositionMap>
): PositionMap {
  const placed: PositionMap = {}
  const taken: TopologyPosition[] = Object.values(positions)
  const pending = nodes.filter((node) => !positions[node.id])
  if (pending.length === 0) return placed

  const neighbors = new Map<number, number[]>()
  for (const edge of edges) {
    const { source, target } = edgeEndpoints(edge)
    if (!neighbors.has(source)) neighbors.set(source, [])
    if (!neighbors.has(target)) neighbors.set(target, [])
    neighbors.get(source)!.push(target)
    neighbors.get(target)!.push(source)
  }

  const positionOf = (id: number) => positions[id] ?? placed[id]
  let bottom = taken.length > 0 ? Math.max(...taken.map((pos) => pos.y)) : 0
  let left = taken.length > 0 ? Math.min(...taken.map((pos) => pos.x)) : 0
  if (taken.length === 0) {
    bottom = -NODE_BOX.height
    left = NODE_BOX.width / 2
  }
  let rowX = left
  const rowY = () => bottom + NODE_BOX.height + 60
  const orphans: TopologyNode[] = []

  // Vários passes: um nó novo ligado a outro nó novo entra depois dele.
  let remaining = [...pending]
  for (let pass = 0; pass < 4 && remaining.length > 0; pass += 1) {
    const next: TopologyNode[] = []
    for (const node of remaining) {
      const anchors = (neighbors.get(node.id) ?? [])
        .map(positionOf)
        .filter((pos): pos is TopologyPosition => Boolean(pos))
      if (anchors.length === 0) {
        next.push(node)
        continue
      }
      const cx = anchors.reduce((sum, pos) => sum + pos.x, 0) / anchors.length
      const cy = anchors.reduce((sum, pos) => sum + pos.y, 0) / anchors.length
      const spot = freeSpotAround({ x: cx, y: cy }, taken)
      placed[node.id] = spot
      taken.push(spot)
    }
    if (next.length === remaining.length) {
      orphans.push(...next)
      remaining = []
    } else {
      remaining = next
    }
  }
  orphans.push(...remaining)

  for (const node of orphans) {
    let spot = { x: rowX, y: rowY() }
    while (!isFree(spot, taken)) {
      rowX += NODE_BOX.width
      spot = { x: rowX, y: rowY() }
    }
    placed[node.id] = spot
    taken.push(spot)
    rowX += NODE_BOX.width
  }
  return placed
}

/** Primeira vaga livre em anéis crescentes, começando embaixo do âncora. */
export function freeSpotAround(
  center: TopologyPosition,
  taken: readonly TopologyPosition[]
): TopologyPosition {
  for (let ring = 1; ring <= 8; ring += 1) {
    const radius = SLOT_DISTANCE * ring
    const slots = 8 * ring
    for (let i = 0; i < slots; i += 1) {
      // Começa em 90° (embaixo) — a borda da rede fica abaixo do núcleo.
      const angle = Math.PI / 2 + (i * 2 * Math.PI) / slots
      const candidate = {
        x: Math.round(center.x + radius * Math.cos(angle)),
        y: Math.round(center.y + radius * Math.sin(angle)),
      }
      if (isFree(candidate, taken)) return candidate
    }
  }
  return { x: Math.round(center.x), y: Math.round(center.y + SLOT_DISTANCE * 9) }
}

// --- Execução no worker ----------------------------------------------------------

type ElkRunner = { layout(graph: ElkNodeJson): Promise<ElkNodeJson> }

let runner: Promise<ElkRunner> | null = null

/**
 * O ELK roda no worker dele próprio (`elk-worker.min.js`): a API fica na
 * thread da tela e só troca mensagens. O `elk.bundled.js` não serve dentro de
 * um worker nosso — ele detecta o ambiente de worker e deixa de exportar.
 */
function getRunner(): Promise<ElkRunner> {
  runner ??= Promise.all([
    import('elkjs/lib/elk-api.js'),
    import('elkjs/lib/elk-worker.min.js?worker'),
  ]).then(([{ default: ELK }, { default: ElkWorker }]) => {
    const elk = new ELK({ workerFactory: () => new ElkWorker() })
    return { layout: (graph) => elk.layout(graph) }
  })
  return runner
}

/** Roda o ELK fora da thread principal e devolve as posições (centros). */
export async function computeLayout(
  kind: LayoutKind,
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[]
): Promise<PositionMap> {
  if (nodes.length === 0) return {}
  if (kind === 'grid') return gridLayout(nodes)
  let plan: ElkPlan
  if (kind === 'hierarchical') {
    plan = buildHierarchy(nodes, edges)
  } else {
    const groups = groupLeaves(nodes, edges)
    plan = { graph: buildElkGraph(kind, nodes, edges, groups), blocks: leafGroupBlocks(groups) }
  }
  const elk = await getRunner()
  return positionsFromElk(await elk.layout(plan.graph), plan.blocks)
}
