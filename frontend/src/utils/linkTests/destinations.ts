/**
 * "De / Até" do teste de velocidade: a partir de um equipamento, para onde dá
 * para medir e o que cada escolha mede. Funções puras sobre o grafo do mapa.
 *
 * - **Enlace direto**: vizinho com IP (ou o equipamento com IP logo depois de
 *   switches sem IP) — mede só aquele enlace (`link`).
 * - **Ponta a ponta**: qualquer outro equipamento — uma medição atravessando o
 *   caminho inteiro (`end_to_end`; `server` quando a origem é o servidor) ou,
 *   pedindo, cada enlace do caminho (`path`, para achar o gargalo).
 *
 * A rota é a do backend (`utils/topology/path`), a mesma que o plano mede.
 */
import type { LinkTestKind } from '@/types/linkTests'
import { pathEdge, routeTree, type PathEdgeSource } from '@/utils/topology/path'
import { linkTargets, type LinkTarget } from './targets'

/** Como medir um destino que não é vizinho direto. */
export type MeasureMode = 'end_to_end' | 'per_hop'

/** O que o seletor "Até" precisa saber de um equipamento. */
export type DestinationNode = {
  id: number
  name: string
  ipAddress?: string | null
  isSystem?: boolean | null
}

export interface Destination {
  deviceId: number
  name: string
  /** Vizinho direto (mede o enlace) ou além (ponta a ponta). */
  direct: boolean
  /** O enlace direto, quando há um só entre os dois. */
  linkId: number | null
  /** Equipamentos e arestas de A até o destino, em ordem (vazio fora do mapa). */
  deviceIds: number[]
  edgeIds: number[]
  /** Os nomes de quem fica no meio, em ordem. */
  viaNames: string[]
  /** O destino está no mapa com um caminho conhecido até ele. */
  mapped: boolean
}

function fromLinkTarget(from: number, target: LinkTarget): Destination {
  return {
    deviceId: target.deviceId,
    name: target.name,
    direct: true,
    linkId: target.linkId,
    deviceIds: [from, ...target.viaDeviceIds, target.deviceId],
    edgeIds: target.edgeIds,
    viaNames: target.viaNames,
    mapped: true,
  }
}

/**
 * Os destinos a partir de `from`: os diretos primeiro (pelo nome), depois os
 * alcançáveis pelo mapa (menos saltos primeiro) e, por fim, os equipamentos
 * que podem ser ponta mas não têm caminho conhecido (`candidates`). A origem
 * nunca é destino; quem não pode ser ponta (`canBeEndpoint`) também não.
 */
export function destinationsFrom(
  from: number,
  nodeOf: (id: number) => DestinationNode | undefined,
  edges: ReadonlyArray<PathEdgeSource>,
  candidates: ReadonlyArray<DestinationNode>,
  canBeEndpoint: (node: DestinationNode) => boolean
): Destination[] {
  const name = (id: number) => nodeOf(id)?.name ?? `#${id}`
  const direct = linkTargets(from, nodeOf, edges)
    .filter((target) => {
      const node = nodeOf(target.deviceId)
      return node === undefined || canBeEndpoint(node)
    })
    .map((target) => fromLinkTarget(from, target))
  const seen = new Set<number>([from, ...direct.map((item) => item.deviceId)])

  const tree = routeTree(edges.map(pathEdge), from)
  const routed: Destination[] = []
  for (const deviceId of tree.reached()) {
    const node = nodeOf(deviceId)
    if (seen.has(deviceId) || !node || !canBeEndpoint(node)) continue
    const route = tree.routeTo(deviceId)
    if (!route) continue
    seen.add(deviceId)
    routed.push({
      deviceId,
      name: node.name,
      direct: false,
      linkId: null,
      deviceIds: route.deviceIds,
      edgeIds: route.edgeIds,
      viaNames: route.deviceIds.slice(1, -1).map(name),
      mapped: true,
    })
  }
  routed.sort(
    (a, b) => a.edgeIds.length - b.edgeIds.length || a.name.localeCompare(b.name, 'pt-BR')
  )

  const unmapped: Destination[] = candidates
    .filter((node) => !seen.has(node.id) && canBeEndpoint(node))
    .map((node) => ({
      deviceId: node.id,
      name: node.name,
      direct: false,
      linkId: null,
      deviceIds: [],
      edgeIds: [],
      viaNames: [],
      mapped: false,
    }))
    .sort((a, b) => a.name.localeCompare(b.name, 'pt-BR'))

  return [...direct, ...routed, ...unmapped]
}

/** O pedido que a escolha gera: o tipo do teste e o enlace, quando há. */
export function measureKind(
  destination: Pick<Destination, 'direct' | 'linkId'>,
  fromSystem: boolean,
  mode: MeasureMode
): { kind: LinkTestKind; linkId: number | null } {
  if (destination.direct) return { kind: 'link', linkId: destination.linkId }
  if (mode === 'per_hop') return { kind: 'path', linkId: null }
  return { kind: fromSystem ? 'server' : 'end_to_end', linkId: null }
}

/** O modo implícito num tipo já escolhido (pedido do histórico, "repetir"). */
export function modeOfKind(kind: LinkTestKind): MeasureMode {
  return kind === 'path' ? 'per_hop' : 'end_to_end'
}

/** "Enlace direto", "3 saltos · por switch › BORDA", "Fora do mapa". */
export function destinationSubtitle(
  destination: Destination,
  via: (names: string[]) => string
): string {
  if (!destination.mapped) return 'Ponta a ponta · sem caminho no mapa'
  if (destination.direct) {
    return destination.viaNames.length > 0
      ? `Enlace direto · passando por ${via(destination.viaNames)}`
      : 'Enlace direto'
  }
  const hops = destination.edgeIds.length
  return `Ponta a ponta · ${hops} ${hops === 1 ? 'salto' : 'saltos'} · por ${via(destination.viaNames)}`
}
