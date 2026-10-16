import { useLinkTestsStore } from '@/stores/linkTests'
import { useTopologyStore } from '@/stores/topology'
import type { LinkTest, LinkTestFlow } from '@/types/linkTests'
import { edgeSpeed } from '@/utils/topology/presentation'

/**
 * O que as telas do teste de enlace precisam saber do resto do sistema: a
 * velocidade nominal de um enlace (para comparar o resultado), a origem da
 * aresta no mapa (para o sentido da animação), o valor ao vivo de cada
 * sentido e o "agora" do teste — a última notícia recebida pelo SSE, sem
 * relógio próprio.
 */
export function useLinkTestContext() {
  const topology = useTopologyStore()
  const store = useLinkTestsStore()

  function speedOf(edgeId: number): number | null {
    const edge = topology.edgesById.get(edgeId)
    return edge ? edgeSpeed(edge) : null
  }

  function edgeSource(edgeId: number): number | null {
    return topology.edgesById.get(edgeId)?.source ?? null
  }

  function liveRate(testId: number) {
    return (legSeq: number, flow: LinkTestFlow) => store.liveBps(testId, legSeq, flow)
  }

  function nowFor(test: Pick<LinkTest, 'id' | 'finishedAt'>): number {
    return store.lastActivityAt(test.id) ?? Date.now()
  }

  return { speedOf, edgeSource, liveRate, nowFor }
}
