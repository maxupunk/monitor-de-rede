/**
 * Menor caminho no grafo do mapa — a mesma régua do backend
 * (`services/topology/path.rs`, `shortest_path`), para que a rota que a tela
 * oferece seja a rota que o backend mede:
 *
 * - grafo **não direcionado**; a hierarquia virtual (id negativo) entra como
 *   aresta, como no `load_path_edges`;
 * - ganha o caminho com **menos saltos**; no empate, a **maior confiança**
 *   somada (confirmado conta 100); no empate restante, a ordem dos ids —
 *   determinístico, independente da ordem em que as arestas chegam.
 *
 * O algoritmo é o mesmo passo a passo (Dijkstra com custo `[saltos,
 * desconfiança]`, fila por `[custo, id]`, vizinhos por `[id, −confiança, id
 * da aresta]`, troca só com custo estritamente menor): o desempate é o do
 * algoritmo, não só a regra, e só copiando o algoritmo ele é idêntico.
 */
import type { TopologyEdge } from '@/stores/topology'
import { edgeEndpoints } from './presentation'

/** O que o caminho precisa saber de uma aresta do mapa. */
export type PathEdgeSource = Pick<
  TopologyEdge,
  'id' | 'source' | 'target' | 'sourceDeviceId' | 'targetDeviceId'
> &
  Partial<Pick<TopologyEdge, 'confidence' | 'confirmed'>>

/** `(id, a, b, confiança 0..100)` — o `PathEdge` do backend. */
export interface PathEdge {
  id: number
  a: number
  b: number
  confidence: number
}

/** Confirmada conta 100; senão, a confiança do enlace em `0..100` (`path_confidence`). */
export function pathConfidence(edge: Pick<PathEdgeSource, 'confidence' | 'confirmed'>): number {
  if (edge.confirmed) return 100
  const value = Number(edge.confidence)
  return Number.isFinite(value) ? Math.trunc(Math.max(0, Math.min(100, value))) : 0
}

export function pathEdge(edge: PathEdgeSource): PathEdge {
  const { source, target } = edgeEndpoints(edge)
  return { id: edge.id, a: source, b: target, confidence: pathConfidence(edge) }
}

/** Caminho de `from` a `to`: `edgeIds[i]` liga `deviceIds[i]` a `deviceIds[i + 1]`. */
export interface TopologyRoute {
  deviceIds: number[]
  edgeIds: number[]
}

/** `[saltos, desconfiança]`, comparado em ordem. */
type Cost = readonly [number, number]

function compareCost(x: Cost, y: Cost): number {
  return x[0] - y[0] || x[1] - y[1]
}

interface Step {
  neighbor: number
  confidence: number
  edgeId: number
}

function adjacency(edges: ReadonlyArray<PathEdge>): Map<number, Step[]> {
  const map = new Map<number, Step[]>()
  const add = (from: number, step: Step) => {
    const list = map.get(from)
    if (list) list.push(step)
    else map.set(from, [step])
  }
  for (const { id, a, b, confidence } of edges) {
    if (a === b) continue
    const capped = Math.min(confidence, 100)
    add(a, { neighbor: b, confidence: capped, edgeId: id })
    add(b, { neighbor: a, confidence: capped, edgeId: id })
  }
  for (const list of map.values()) {
    list.sort(
      (x, y) => x.neighbor - y.neighbor || y.confidence - x.confidence || x.edgeId - y.edgeId
    )
  }
  return map
}

/** Fila de prioridade mínima por `[custo, id]` (o `BinaryHeap<Reverse<…>>` do backend). */
class Queue {
  private readonly items: Array<{ cost: Cost; device: number }> = []

  private less(i: number, j: number): boolean {
    const x = this.items[i]
    const y = this.items[j]
    return (compareCost(x.cost, y.cost) || x.device - y.device) < 0
  }

  private swap(i: number, j: number) {
    const tmp = this.items[i]
    this.items[i] = this.items[j]
    this.items[j] = tmp
  }

  get size(): number {
    return this.items.length
  }

  push(cost: Cost, device: number) {
    this.items.push({ cost, device })
    let i = this.items.length - 1
    while (i > 0) {
      const parent = (i - 1) >> 1
      if (!this.less(i, parent)) break
      this.swap(i, parent)
      i = parent
    }
  }

  pop(): { cost: Cost; device: number } | undefined {
    const top = this.items[0]
    const last = this.items.pop()
    if (this.items.length > 0 && last) {
      this.items[0] = last
      let i = 0
      for (;;) {
        const left = 2 * i + 1
        const right = left + 1
        let smallest = i
        if (left < this.items.length && this.less(left, smallest)) smallest = left
        if (right < this.items.length && this.less(right, smallest)) smallest = right
        if (smallest === i) break
        this.swap(i, smallest)
        i = smallest
      }
    }
    return top
  }
}

/** Os menores caminhos a partir de uma origem. */
export interface RouteTree {
  /** O caminho até `to`, ou `null` sem ligação conhecida. */
  routeTo(to: number): TopologyRoute | null
  /** Os equipamentos alcançados (sem a origem). */
  reached(): number[]
}

/**
 * Menores caminhos de `from` a todos os equipamentos. `expand` diz por quem
 * o caminho pode **passar** (a origem sempre passa): um equipamento que não
 * passa ainda é alcançado, mas é ponta final.
 *
 * Com `expand` = "só transparentes", a rota até cada ponta é exatamente a
 * do backend no recorte `segment_edges(from, to)` + `shortest_path`: as
 * pontas que não passam nunca mudam o custo nem a ordem de quem passa.
 */
export function routeTree(
  edges: ReadonlyArray<PathEdge>,
  from: number,
  expand: (deviceId: number) => boolean = () => true
): RouteTree {
  const graph = adjacency(edges)
  const best = new Map<number, Cost>([[from, [0, 0]]])
  const previous = new Map<number, { device: number; edgeId: number }>()
  const queue = new Queue()
  queue.push([0, 0], from)

  while (queue.size > 0) {
    const { cost, device } = queue.pop()!
    const known = best.get(device)
    if (known && compareCost(known, cost) < 0) continue
    if (device !== from && !expand(device)) continue
    for (const { neighbor, confidence, edgeId } of graph.get(device) ?? []) {
      const next: Cost = [cost[0] + 1, cost[1] + (100 - confidence)]
      const seen = best.get(neighbor)
      if (seen === undefined || compareCost(next, seen) < 0) {
        best.set(neighbor, next)
        previous.set(neighbor, { device, edgeId })
        queue.push(next, neighbor)
      }
    }
  }

  return {
    routeTo(to) {
      if (to === from) return { deviceIds: [from], edgeIds: [] }
      if (!previous.has(to)) return null
      const deviceIds = [to]
      const edgeIds: number[] = []
      let cursor = to
      for (let step = previous.get(cursor); step; step = previous.get(cursor)) {
        edgeIds.push(step.edgeId)
        deviceIds.push(step.device)
        cursor = step.device
        if (cursor === from) break
      }
      return { deviceIds: deviceIds.reverse(), edgeIds: edgeIds.reverse() }
    },
    reached() {
      return [...previous.keys()]
    },
  }
}
