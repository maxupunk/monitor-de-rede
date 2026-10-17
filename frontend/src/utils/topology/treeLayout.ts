/**
 * Árvore compacta do layout hierárquico — funções PURAS, sem ELK.
 *
 * Uma rede real é quase uma árvore: núcleo → distribuição → acesso → ponta.
 * O ELK em camadas põe cada nível numa fileira só, e 100 switches de acesso
 * viram uma faixa de 15.000 px com arestas diagonais cruzando o mapa. Aqui a
 * parte que é árvore de verdade (sem enlace redundante) vira um bloco
 * arrumado à mão, no estilo organograma:
 *
 * - o pai fica centrado acima dos filhos;
 * - folhas (PCs, câmeras…) de um mesmo pai viram uma grade;
 * - muitos filhos quebram em fileiras com um "corredor" (espinha) no meio,
 *   por onde desce o tronco das arestas — as arestas ortogonais (`taxi`) do
 *   canvas descem pelo corredor e entram em cada filho por cima, sem cortar
 *   nenhum bloco.
 *
 * Só o que tem ciclo (núcleo com enlaces redundantes) fica para o ELK, que
 * minimiza cruzamentos onde eles existem de fato.
 */
import type { TopologyEdge, TopologyNode, TopologyPosition } from '@/stores/topology'
import { edgeEndpoints, nodeCategory } from './presentation'

/** Tamanho que cada nó ocupa no layout (ícone + rótulo de duas linhas). */
export const NODE_BOX = { width: 124, height: 84 }

/** Do pai à fileira dos filhos (espaço do barramento das arestas). */
export const LEVEL_GAP = 64
/** Entre fileiras de uma grade de folhas (o barramento passa no vão). */
export const LEAF_ROW_HEIGHT = NODE_BOX.height + 30
/** Entre blocos irmãos na mesma fileira. */
const UNIT_GAP = 28
/** Entre fileiras de blocos irmãos. */
const ROW_GAP = 56
/** Corredor central por onde desce o tronco quando os filhos quebram em fileiras. */
const SPINE = 44
/**
 * Até esta largura os filhos ficam numa fileira só. Não é fixa: `arrangeCompact`
 * experimenta vários limites e fica com o que melhor cabe numa tela.
 */
export interface ArrangeOptions {
  rowMax: number
}
const ROW_LIMITS = [Number.POSITIVE_INFINITY, 5200, 3600, 2600, 1800, 1300, 900]
/** Tela de referência para escolher o limite (paisagem, como a maioria dos monitores). */
const VIEW = { width: 1600, height: 1000 }
/** Proporção alvo (largura/altura) de um conjunto de blocos que quebra em fileiras. */
const TARGET_ASPECT = 1.6
/** A partir de quantas folhas no mesmo pai elas viram grade. */
export const LEAF_GROUP_MIN = 3
const LEAF_MAX_COLS = 6

export const CATEGORY_ORDER = [
  'router',
  'firewall',
  'switch',
  'ap',
  'server',
  'nas',
  'workstation',
  'printer',
  'camera',
  'voip',
  'iot',
  'media',
  'mobile',
  'ups',
  'web_device',
  'other',
  'unknown',
]

/** Ordem estável: servidor, infraestrutura, depois a ponta; nome com números naturais. */
export function compareNodes(a: TopologyNode, b: TopologyNode): number {
  if (Boolean(a.isSystem) !== Boolean(b.isSystem)) return a.isSystem ? -1 : 1
  const ca = CATEGORY_ORDER.indexOf(nodeCategory(a))
  const cb = CATEGORY_ORDER.indexOf(nodeCategory(b))
  return ca - cb || a.name.localeCompare(b.name, 'pt-BR', { numeric: true }) || a.id - b.id
}

/** Bloco já arrumado: centros dos nós relativos ao canto superior esquerdo. */
export interface TreeBox {
  width: number
  height: number
  positions: Map<number, TopologyPosition>
}

interface Unit extends TreeBox {
  /** X do ponto onde o pai liga o bloco (centro do nó-raiz, ou do conjunto). */
  anchorX: number
}

// --- Floresta geradora ---------------------------------------------------------

export interface SpanningForest {
  parent: Map<number, number | null>
  children: Map<number, number[]>
  /** Nós que tocam algum enlace que não é pai↔filho (redundância, ciclo). */
  crossLinked: Set<number>
  /** Nós sem nenhum enlace. */
  isolated: number[]
}

/**
 * Busca em largura a partir das raízes (uma por componente). Enlace entre pai
 * e filho repetido (LAG, dois cabos) não conta como ciclo.
 */
export function spanningForest(
  nodes: readonly TopologyNode[],
  edges: readonly TopologyEdge[],
  roots: readonly number[]
): SpanningForest {
  const byId = new Map(nodes.map((node) => [node.id, node]))
  const adjacency = new Map<number, number[]>()
  const pairs: Array<[number, number]> = []
  for (const edge of edges) {
    const { source, target } = edgeEndpoints(edge)
    if (source === target || !byId.has(source) || !byId.has(target)) continue
    if (!adjacency.has(source)) adjacency.set(source, [])
    if (!adjacency.has(target)) adjacency.set(target, [])
    adjacency.get(source)!.push(target)
    adjacency.get(target)!.push(source)
    pairs.push([source, target])
  }
  const order = (ids: number[]) =>
    [...new Set(ids)].sort((a, b) => compareNodes(byId.get(a)!, byId.get(b)!))

  const parent = new Map<number, number | null>()
  const children = new Map<number, number[]>()
  const visit = (root: number) => {
    parent.set(root, null)
    const queue = [root]
    while (queue.length > 0) {
      const current = queue.shift()!
      const kids: number[] = []
      for (const other of order(adjacency.get(current) ?? [])) {
        if (parent.has(other)) continue
        parent.set(other, current)
        kids.push(other)
        queue.push(other)
      }
      children.set(current, kids)
    }
  }
  for (const root of roots) if (byId.has(root) && !parent.has(root)) visit(root)
  const isolated: number[] = []
  for (const node of [...nodes].sort(compareNodes)) {
    if (parent.has(node.id)) continue
    if (!adjacency.has(node.id)) {
      isolated.push(node.id)
      continue
    }
    visit(node.id)
  }

  const crossLinked = new Set<number>()
  for (const [a, b] of pairs) {
    if (parent.get(a) === b || parent.get(b) === a) continue
    crossLinked.add(a)
    crossLinked.add(b)
  }
  return { parent, children, crossLinked, isolated }
}

/**
 * Nós cuja subárvore inteira é árvore pura (nenhum nó dela tem enlace
 * redundante). Esses viram blocos compactos; o resto é o núcleo do ELK.
 */
export function closedNodes(forest: SpanningForest): Set<number> {
  const closed = new Set<number>()
  const check = (id: number): boolean => {
    let ok = !forest.crossLinked.has(id)
    for (const child of forest.children.get(id) ?? []) ok = check(child) && ok
    if (ok) closed.add(id)
    return ok
  }
  for (const [id, parent] of forest.parent) if (parent === null) check(id)
  return closed
}

// --- Arrumação ----------------------------------------------------------------------

function single(id: number): Unit {
  return {
    width: NODE_BOX.width,
    height: NODE_BOX.height,
    positions: new Map([[id, { x: NODE_BOX.width / 2, y: NODE_BOX.height / 2 }]]),
    anchorX: NODE_BOX.width / 2,
  }
}

/** Colunas da grade de folhas: par quando há mais de uma fileira (o tronco desce no meio). */
export function leafColumns(count: number): number {
  let cols = Math.min(LEAF_MAX_COLS, Math.max(1, Math.ceil(Math.sqrt(count * 1.6))))
  if (Math.ceil(count / cols) > 1 && cols % 2 === 1) cols = Math.min(LEAF_MAX_COLS, cols + 1)
  return Math.min(cols, count)
}

/** Grade de folhas, centrada no ponto de ligação. */
export function leafGrid(leaves: readonly number[]): Unit {
  const cols = leafColumns(leaves.length)
  const rows = Math.ceil(leaves.length / cols)
  const positions = new Map<number, TopologyPosition>()
  leaves.forEach((id, index) => {
    positions.set(id, {
      x: (index % cols) * NODE_BOX.width + NODE_BOX.width / 2,
      y: Math.floor(index / cols) * LEAF_ROW_HEIGHT + NODE_BOX.height / 2,
    })
  })
  const width = cols * NODE_BOX.width
  return {
    width,
    height: (rows - 1) * LEAF_ROW_HEIGHT + NODE_BOX.height,
    positions,
    anchorX: width / 2,
  }
}

function place(target: Map<number, TopologyPosition>, unit: TreeBox, dx: number, dy: number) {
  for (const [id, pos] of unit.positions) target.set(id, { x: pos.x + dx, y: pos.y + dy })
}

/**
 * Põe os blocos irmãos lado a lado; quando passam da largura, quebra em
 * fileiras com o corredor central livre (metade à esquerda, metade à direita).
 */
export function packUnits(units: readonly Unit[], options: ArrangeOptions): Unit {
  const positions = new Map<number, TopologyPosition>()
  const total = units.reduce((sum, unit) => sum + unit.width, 0) + UNIT_GAP * (units.length - 1)
  if (units.length <= 1 || total <= options.rowMax) {
    let x = 0
    let height = 0
    for (const unit of units) {
      place(positions, unit, x, 0)
      x += unit.width + UNIT_GAP
      height = Math.max(height, unit.height)
    }
    return { width: total, height, positions, anchorX: total / 2 }
  }

  const avgWidth = units.reduce((sum, unit) => sum + unit.width, 0) / units.length
  const avgHeight = units.reduce((sum, unit) => sum + unit.height, 0) / units.length
  let cols = Math.round(Math.sqrt((units.length * TARGET_ASPECT * avgHeight) / avgWidth))
  cols = Math.max(2, Math.min(units.length, cols))
  if (cols % 2 === 1 && cols < units.length) cols += 1
  const rowCount = Math.ceil(units.length / cols)
  cols = Math.ceil(units.length / rowCount)

  const rows: Unit[][] = []
  for (let i = 0; i < units.length; i += cols) rows.push(units.slice(i, i + cols))
  const halves = rows.map((row) => {
    const split = Math.ceil(row.length / 2)
    const span = (list: Unit[]) =>
      list.reduce((sum, unit) => sum + unit.width, 0) + UNIT_GAP * Math.max(0, list.length - 1)
    return { left: row.slice(0, split), right: row.slice(split), span }
  })
  const half = Math.max(...halves.map(({ left, right, span }) => Math.max(span(left), span(right))))
  const width = half * 2 + SPINE
  const center = width / 2
  let y = 0
  for (const { left, right } of halves) {
    let x = center - SPINE / 2
    for (const unit of [...left].reverse()) {
      x -= unit.width
      place(positions, unit, x, y)
      x -= UNIT_GAP
    }
    x = center + SPINE / 2
    for (const unit of right) {
      place(positions, unit, x, y)
      x += unit.width + UNIT_GAP
    }
    y += Math.max(...[...left, ...right].map((unit) => unit.height)) + ROW_GAP
  }
  return { width, height: y - ROW_GAP, positions, anchorX: center }
}

/** Filhos de um nó já arrumados num conjunto (folhas em grade, subárvores em blocos). */
export function childrenPack(
  kids: readonly number[],
  children: ReadonlyMap<number, readonly number[]>,
  options: ArrangeOptions
): Unit | null {
  if (kids.length === 0) return null
  const leaves = kids.filter((id) => (children.get(id) ?? []).length === 0)
  const units: Unit[] = []
  if (leaves.length >= LEAF_GROUP_MIN) units.push(leafGrid(leaves))
  else units.push(...leaves.map(single))
  for (const id of kids) {
    if ((children.get(id) ?? []).length > 0) units.push(arrangeSubtree(id, children, options))
  }
  return packUnits(units, options)
}

/** Subárvore inteira num bloco: o nó no topo, centrado sobre os filhos. */
export function arrangeSubtree(
  root: number,
  children: ReadonlyMap<number, readonly number[]>,
  options: ArrangeOptions
): Unit {
  const pack = childrenPack(children.get(root) ?? [], children, options)
  if (!pack) return single(root)
  const width = Math.max(NODE_BOX.width, pack.width)
  const positions = new Map<number, TopologyPosition>()
  const anchorX = (width - pack.width) / 2 + pack.anchorX
  positions.set(root, { x: anchorX, y: NODE_BOX.height / 2 })
  place(positions, pack, (width - pack.width) / 2, NODE_BOX.height + LEVEL_GAP)
  return { width, height: NODE_BOX.height + LEVEL_GAP + pack.height, positions, anchorX }
}

/**
 * Escolhe, entre vários limites de fileira, o arranjo que fica maior numa
 * tela de referência — árvore larga e rasa ou compacta em fileiras, o que
 * render mais zoom. Empate (até 2%) fica com menos quebras: mais cara de árvore.
 */
export function arrangeCompact(make: (options: ArrangeOptions) => Unit | null): Unit | null {
  let best: Unit | null = null
  let bestScore = 0
  for (const rowMax of ROW_LIMITS) {
    const unit = make({ rowMax })
    if (!unit) return null
    const score = Math.min(VIEW.width / unit.width, VIEW.height / unit.height)
    if (!best || score > bestScore * 1.02) {
      best = unit
      bestScore = score
    }
  }
  return best
}

/** Nós sem enlace: uma grade só, em vez de um bloco solto por nó. */
export function isolatedGrid(ids: readonly number[]): TreeBox | null {
  if (ids.length === 0) return null
  const cols = Math.max(1, Math.min(10, Math.ceil(Math.sqrt(ids.length * 2))))
  const positions = new Map<number, TopologyPosition>()
  ids.forEach((id, index) => {
    positions.set(id, {
      x: (index % cols) * NODE_BOX.width + NODE_BOX.width / 2,
      y: Math.floor(index / cols) * (NODE_BOX.height + 24) + NODE_BOX.height / 2,
    })
  })
  const rows = Math.ceil(ids.length / cols)
  return {
    width: Math.min(ids.length, cols) * NODE_BOX.width,
    height: (rows - 1) * (NODE_BOX.height + 24) + NODE_BOX.height,
    positions,
  }
}
