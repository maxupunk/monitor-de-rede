/**
 * Tradução pura de um teste de enlace na camada visual da topologia
 * (`stores/topologyOverlay`): caminho destacado, estado de cada enlace
 * (pendente → em teste, com a animação no sentido medido → resultado) e o
 * rótulo com os Mbps. Pernas ponta a ponta não têm aresta: marcam só as pontas.
 * Perna que atravessa switches sem IP marca todas as arestas do trecho; o
 * rótulo fica só na primeira.
 */
import type {
  EdgeOverlay,
  NodeOverlay,
  OverlayFlow,
  OverlayState,
  PathHighlight,
} from '@/stores/topologyOverlay'
import type {
  LinkTest,
  LinkTestFlow,
  LinkTestLeg,
  LinkTestPlan,
  LinkTestPlanLeg,
  LinkTestStepStatus,
} from '@/types/linkTests'
import { flowPairLabel, legEdgeIds } from './presentation'

export interface LinkTestOverlay {
  highlight: PathHighlight
  edges: Record<number, EdgeOverlay>
  nodes: Record<number, NodeOverlay>
}

/** Valor ao vivo de um sentido da perna (última amostra), ou `null`. */
export type LiveRate = (legSeq: number, flow: LinkTestFlow) => number | null

/** Origem (no canvas) da aresta — para inverter o sentido da animação. */
export type EdgeSource = (edgeId: number) => number | null

export function overlayState(status: LinkTestStepStatus): OverlayState {
  switch (status) {
    case 'running':
      return 'running'
    case 'succeeded':
      return 'succeeded'
    case 'failed':
      return 'failed'
    case 'pending':
      return 'pending'
    default:
      // Pulada ou cancelada: fica marcada, mas sem cor de erro.
      return 'skipped'
  }
}

/** Sentido (da perna) sendo medido agora. */
export function runningLegFlow(leg: Pick<LinkTestLeg, 'stages'>): LinkTestFlow | null {
  const running = leg.stages.find((stage) => stage.status === 'running')
  if (running?.key === 'forward') return 'forward'
  if (running?.key === 'reverse') return 'reverse'
  return null
}

/**
 * Sentido da animação no canvas para a aresta `index` da perna. A perna mede
 * A→B (`forward`); a aresta `index` liga o `index`-ésimo equipamento da
 * cadeia A, switches…, B ao seguinte. Se o mapa a desenhou ao contrário, o
 * mesmo fluxo anda ao contrário.
 */
export function canvasFlow(
  leg: Pick<LinkTestLeg, 'stages' | 'edgeId' | 'edgeIds' | 'viaDeviceIds' | 'a' | 'b'>,
  edgeSource: EdgeSource = () => null,
  index = 0
): OverlayFlow {
  const flow = runningLegFlow(leg)
  if (flow === null) return 'both'
  const edgeId = legEdgeIds(leg)[index] ?? null
  const chain = [leg.a.deviceId, ...(leg.viaDeviceIds ?? []), leg.b.deviceId]
  const near = chain[index] ?? null
  const far = chain[index + 1] ?? null
  const source = edgeId !== null ? edgeSource(edgeId) : null
  const inverted = source !== null && source === far && source !== near
  if (!inverted) return flow
  return flow === 'forward' ? 'reverse' : 'forward'
}

/** Pílula do enlace: Mbps ao vivo enquanto mede, o resultado no fim. */
export function legOverlayLabel(leg: LinkTestLeg, live: LiveRate = () => null): string | undefined {
  if (leg.status === 'running') {
    const forward = live(leg.seq, 'forward') ?? leg.forwardBps
    const reverse = live(leg.seq, 'reverse') ?? leg.reverseBps
    return flowPairLabel(forward, reverse) ?? 'Preparando…'
  }
  if (leg.status === 'succeeded') return flowPairLabel(leg.forwardBps, leg.reverseBps) ?? undefined
  if (leg.status === 'failed') return 'Falhou'
  if (leg.status === 'skipped') return 'Pulado'
  if (leg.status === 'cancelled') return 'Cancelado'
  return undefined
}

type PathLeg = Pick<LinkTestPlanLeg, 'a' | 'b' | 'edgeId' | 'edgeIds' | 'viaDeviceIds'>

/** As pontas e os switches sem IP atravessados, em ordem. */
function nodeIds(legs: ReadonlyArray<PathLeg>): number[] {
  const ids: number[] = []
  for (const leg of legs) {
    if (leg.a.deviceId !== null) ids.push(leg.a.deviceId)
    ids.push(...(leg.viaDeviceIds ?? []))
    if (leg.b.deviceId !== null) ids.push(leg.b.deviceId)
  }
  return [...new Set(ids)]
}

/** Nós e arestas do caminho (para destacar e enquadrar). */
export function pathOf(legs: ReadonlyArray<PathLeg>): PathHighlight {
  return {
    nodeIds: nodeIds(legs),
    edgeIds: [...new Set(legs.flatMap(legEdgeIds))],
  }
}

export function overlayForTest(
  test: Pick<LinkTest, 'legs'>,
  live: LiveRate = () => null,
  edgeSource: EdgeSource = () => null
): LinkTestOverlay {
  const edges: Record<number, EdgeOverlay> = {}
  const nodes: Record<number, NodeOverlay> = {}
  for (const leg of test.legs) {
    const state = overlayState(leg.status)
    const label = legOverlayLabel(leg, live)
    const legEdges = legEdgeIds(leg)
    if (legEdges.length > 0) {
      legEdges.forEach((edgeId, index) => {
        edges[edgeId] = {
          state,
          ...(label && index === 0 ? { label } : {}),
          ...(state === 'running' ? { flow: canvasFlow(leg, edgeSource, index) } : {}),
        }
      })
    } else if (leg.kind === 'end_to_end') {
      for (const id of [leg.a.deviceId, leg.b.deviceId]) {
        if (id !== null) nodes[id] = { state, ...(label ? { label } : {}) }
      }
    }
  }
  return { highlight: pathOf(test.legs), edges, nodes }
}

/** Antes de rodar: o caminho do plano destacado e os enlaces pendentes. */
export function overlayForPlan(plan: Pick<LinkTestPlan, 'legs'>): LinkTestOverlay {
  const edges: Record<number, EdgeOverlay> = {}
  for (const leg of plan.legs) {
    for (const edgeId of legEdgeIds(leg)) {
      edges[edgeId] = { state: leg.runnable ? 'pending' : 'skipped' }
    }
  }
  return { highlight: pathOf(plan.legs), edges, nodes: {} }
}
