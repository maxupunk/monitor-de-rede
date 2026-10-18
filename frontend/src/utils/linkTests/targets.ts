/**
 * Equipamentos sem IP no teste de enlace (ADR 015 §5.2) — a mesma régua do
 * backend (`services/link_tests/transparent.rs`): sem IP o equipamento é
 * **transparente** (switch não gerenciável, hub). Ele não mede nada, mas o
 * tráfego passa por ele: o teste vai de um equipamento com IP ao próximo com
 * IP, atravessando os transparentes do meio.
 *
 * Funções puras sobre o grafo do mapa: quem é transparente, os destinos de
 * "Enlace" a partir de um equipamento e qual opção o rascunho escolheu.
 */
import type { TopologyEdge, TopologyNode } from '@/stores/topology'
import { edgeEndpoints, isVirtualEdge } from '@/utils/topology/presentation'

/** O que a régua precisa saber de um equipamento (nó do mapa ou cadastro). */
export type TransparencySubject = {
  ipAddress?: string | null
  isSystem?: boolean | null
}

/** Sem IP é transparente. O próprio NetMonitor nunca é: ele é a central. */
export function isLinkTestTransparent(device: TransparencySubject): boolean {
  if (device.isSystem) return false
  return !device.ipAddress || device.ipAddress.trim() === ''
}

/** Dica dos seletores de equipamento, que não oferecem os transparentes. */
export const TRANSPARENT_HINT = 'Switches sem IP não entram no teste — o enlace passa por eles.'

/** "switch-a › switch-b": os equipamentos sem IP atravessados, em ordem. */
export function viaText(names: ReadonlyArray<string>): string | null {
  return names.length > 0 ? names.join(' › ') : null
}

type NodeLike = Pick<TopologyNode, 'id' | 'name'> & TransparencySubject
type EdgeLike = Pick<
  TopologyEdge,
  'id' | 'linkType' | 'source' | 'target' | 'sourceDeviceId' | 'targetDeviceId'
>

/** Um destino de "Enlace" a partir do equipamento escolhido. */
export interface LinkTarget {
  /** `edge:<id>` (enlace direto) ou `device:<id>` (através de switches sem IP). */
  key: string
  deviceId: number
  name: string
  /** O enlace, quando é direto; `null` quando atravessa equipamentos sem IP. */
  linkId: number | null
  /** As arestas do trecho, em ordem a partir da origem. */
  edgeIds: number[]
  viaDeviceIds: number[]
  viaNames: string[]
}

interface Step {
  neighbor: number
  edgeId: number
}

function adjacency(edges: ReadonlyArray<EdgeLike>): Map<number, Step[]> {
  const map = new Map<number, Step[]>()
  const add = (from: number, step: Step) => {
    const list = map.get(from)
    if (list) list.push(step)
    else map.set(from, [step])
  }
  // Hierarquia virtual não é cabo: não se mede nem se atravessa.
  for (const edge of edges) {
    if (isVirtualEdge(edge)) continue
    const { source, target } = edgeEndpoints(edge)
    if (source === target) continue
    add(source, { neighbor: target, edgeId: edge.id })
    add(target, { neighbor: source, edgeId: edge.id })
  }
  for (const list of map.values()) list.sort((a, b) => a.edgeId - b.edgeId)
  return map
}

/**
 * Os destinos testáveis de "Enlace" a partir de `from`: os vizinhos com IP
 * (enlace direto) e, atrás de cada vizinho sem IP, os primeiros equipamentos
 * com IP alcançados só por equipamentos sem IP (busca em largura: o trecho
 * mais curto fica). Um destino que também é vizinho direto fica só como
 * direto. Ordem: pelo nome.
 */
export function linkTargets(
  from: number,
  nodeOf: (id: number) => NodeLike | undefined,
  edges: ReadonlyArray<EdgeLike>
): LinkTarget[] {
  const graph = adjacency(edges)
  const name = (id: number) => nodeOf(id)?.name ?? `#${id}`
  // Nó fora do mapa conta como equipamento com IP: o backend decide.
  const transparent = (id: number) => {
    const node = nodeOf(id)
    return node !== undefined && isLinkTestTransparent(node)
  }

  const direct: LinkTarget[] = []
  for (const { neighbor, edgeId } of graph.get(from) ?? []) {
    if (neighbor === from || transparent(neighbor)) continue
    direct.push({
      key: `edge:${edgeId}`,
      deviceId: neighbor,
      name: name(neighbor),
      linkId: edgeId,
      edgeIds: [edgeId],
      viaDeviceIds: [],
      viaNames: [],
    })
  }

  const reached = new Set(direct.map((target) => target.deviceId))
  const beyond: LinkTarget[] = []
  const visited = new Set<number>([from])
  const queue: Array<{ device: number; edgeIds: number[]; via: number[] }> = [
    { device: from, edgeIds: [], via: [] },
  ]
  while (queue.length > 0) {
    const current = queue.shift()!
    for (const { neighbor, edgeId } of graph.get(current.device) ?? []) {
      if (visited.has(neighbor)) continue
      const edgeIds = [...current.edgeIds, edgeId]
      if (transparent(neighbor)) {
        visited.add(neighbor)
        queue.push({ device: neighbor, edgeIds, via: [...current.via, neighbor] })
        continue
      }
      if (current.via.length === 0 || reached.has(neighbor)) continue
      reached.add(neighbor)
      beyond.push({
        key: `device:${neighbor}`,
        deviceId: neighbor,
        name: name(neighbor),
        linkId: null,
        edgeIds,
        viaDeviceIds: current.via,
        viaNames: current.via.map(name),
      })
    }
  }

  return [...direct, ...beyond].sort(
    (a, b) => a.name.localeCompare(b.name, 'pt-BR') || a.key.localeCompare(b.key)
  )
}

/** A opção que o rascunho escolheu (`null` quando ainda não há destino). */
export function selectedTargetKey(
  draft: { linkId: number | null; targetDeviceId: number | null },
  targets: ReadonlyArray<Pick<LinkTarget, 'key' | 'deviceId' | 'linkId'>>
): string | null {
  if (draft.linkId !== null) return `edge:${draft.linkId}`
  if (draft.targetDeviceId === null) return null
  const found = targets.find((target) => target.deviceId === draft.targetDeviceId)
  return found?.key ?? `device:${draft.targetDeviceId}`
}

/**
 * Para um enlace do mapa com uma ponta sem IP: a ponta com IP, de onde o
 * assistente abre para escolher o destino depois do switch. `null` quando as
 * duas pontas têm IP (enlace direto) ou nenhuma tem (nada a medir ali).
 */
export function transparentEdgeOrigin(
  edge: EdgeLike,
  nodeOf: (id: number) => TransparencySubject | undefined
): { origin: number | null; direct: boolean } {
  const { source, target } = edgeEndpoints(edge)
  const through = (id: number) => {
    const node = nodeOf(id)
    return node !== undefined && isLinkTestTransparent(node)
  }
  const [sourceThrough, targetThrough] = [through(source), through(target)]
  if (!sourceThrough && !targetThrough) return { origin: null, direct: true }
  if (sourceThrough && targetThrough) return { origin: null, direct: false }
  return { origin: sourceThrough ? target : source, direct: false }
}
