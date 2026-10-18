/**
 * Equipamentos sem IP no teste de enlace (ADR 015 §5.2) — a mesma régua do
 * backend (`services/link_tests/transparent.rs`): sem IP o equipamento é
 * **transparente** (switch não gerenciável, hub). Ele não mede nada, mas o
 * tráfego passa por ele: o teste vai de um equipamento com IP ao próximo com
 * IP, atravessando os transparentes do meio.
 *
 * Funções puras sobre o grafo do mapa: quem é transparente, os destinos de
 * "Enlace" a partir de um equipamento e qual opção o rascunho escolheu. A
 * legenda "a › b" é de `presentation.ts` (`viaText`).
 */
import type { TopologyNode } from '@/stores/topology'
import { pathEdge, routeTree, type PathEdgeSource } from '@/utils/topology/path'
import { edgeEndpoints, isUnmanagedSwitch } from '@/utils/topology/presentation'

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

/**
 * Pode ser ponta de um teste (seletores de equipamento): nem transparente,
 * nem switch não gerenciável (que, mesmo com IP cadastrado, não roda plugin).
 */
export function isLinkTestEndpoint(device: TransparencySubject & { type: string }): boolean {
  return !isUnmanagedSwitch(device) && !isLinkTestTransparent(device)
}

/** Dica dos seletores de equipamento, que não oferecem os transparentes. */
export const TRANSPARENT_HINT = 'Switches sem IP não entram no teste — o enlace passa por eles.'

/**
 * Transparência por id, a partir do mapa. Nó fora do mapa conta como
 * equipamento com IP: o backend decide.
 */
function transparentIn(
  nodeOf: (id: number) => TransparencySubject | undefined
): (id: number) => boolean {
  return (id) => {
    const node = nodeOf(id)
    return node !== undefined && isLinkTestTransparent(node)
  }
}

type NodeLike = Pick<TopologyNode, 'id' | 'name'> & TransparencySubject

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

/**
 * Os destinos testáveis de "Enlace" a partir de `from`: os vizinhos com IP
 * (cada enlace direto, a hierarquia declarada inclusive) e, atrás dos
 * vizinhos sem IP, os equipamentos com IP alcançados só por equipamentos sem
 * IP. A rota até cada um é a do backend (`utils/topology/path`: menos
 * saltos, depois maior confiança, depois os ids) — a opção mostrada é o
 * trecho medido. Um destino com enlace direto fica só como direto. Ordem:
 * pelo nome.
 */
export function linkTargets(
  from: number,
  nodeOf: (id: number) => NodeLike | undefined,
  edges: ReadonlyArray<PathEdgeSource>
): LinkTarget[] {
  const name = (id: number) => nodeOf(id)?.name ?? `#${id}`
  const transparent = transparentIn(nodeOf)
  const graph = edges.map(pathEdge)

  const direct: LinkTarget[] = []
  for (const edge of graph) {
    const neighbor = edge.a === from ? edge.b : edge.b === from ? edge.a : null
    if (neighbor === null || neighbor === from || transparent(neighbor)) continue
    direct.push({
      key: `edge:${edge.id}`,
      deviceId: neighbor,
      name: name(neighbor),
      linkId: edge.id,
      edgeIds: [edge.id],
      viaDeviceIds: [],
      viaNames: [],
    })
  }

  // Só a origem e os transparentes são atravessados: cada equipamento com IP
  // alcançado é ponta, como no recorte `segment_edges` do backend.
  const tree = routeTree(graph, from, transparent)
  const beyond: LinkTarget[] = []
  for (const deviceId of tree.reached()) {
    if (transparent(deviceId)) continue
    const route = tree.routeTo(deviceId)
    const via = route ? route.deviceIds.slice(1, -1) : []
    if (!route || via.length === 0) continue
    beyond.push({
      key: `device:${deviceId}`,
      deviceId,
      name: name(deviceId),
      linkId: null,
      edgeIds: route.edgeIds,
      viaDeviceIds: via,
      viaNames: via.map(name),
    })
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
  edge: PathEdgeSource,
  nodeOf: (id: number) => TransparencySubject | undefined
): { origin: number | null; direct: boolean } {
  const { source, target } = edgeEndpoints(edge)
  const through = transparentIn(nodeOf)
  const [sourceThrough, targetThrough] = [through(source), through(target)]
  if (!sourceThrough && !targetThrough) return { origin: null, direct: true }
  if (sourceThrough && targetThrough) return { origin: null, direct: false }
  return { origin: sourceThrough ? target : source, direct: false }
}
