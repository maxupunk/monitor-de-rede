/**
 * Medidores de ida e volta de uma perna: o valor ao vivo enquanto mede, o
 * final depois, e a comparação com a velocidade da porta. Puro — a perna da
 * lista de detalhes e a tela do teste mostram os mesmos números.
 */
import type { LinkTestFlow, LinkTestLeg, LinkTestSample } from '@/types/linkTests'
import { runningLegFlow } from './overlay'
import { compactBps, endpointLabel } from './presentation'
import { resultVsSpeed, type SpeedComparison } from './results'

export interface Gauge {
  flow: LinkTestFlow
  label: string
  /** "AP → BORDA". */
  route: string
  icon: string
  value: string
  /** O número em bps (o valor mostrado), ou `null`. */
  bps: number | null
  /** Este sentido está sendo medido agora. */
  active: boolean
  comparison: SpeedComparison | null
}

function lastBps(list: ReadonlyArray<LinkTestSample>): number | null {
  return list.length > 0 ? list[list.length - 1].bps : null
}

export function legGauges(
  leg: LinkTestLeg,
  direction: 'both' | 'forward' | 'reverse',
  forwardSamples: ReadonlyArray<LinkTestSample>,
  reverseSamples: ReadonlyArray<LinkTestSample>,
  speedBps: number | null
): Gauge[] {
  const running = leg.status === 'running'
  if (!running && leg.forwardBps === null && leg.reverseBps === null) return []
  const flows: LinkTestFlow[] = direction === 'both' ? ['forward', 'reverse'] : [direction]
  const measuring = running ? runningLegFlow(leg) : null
  return flows.map((flow) => {
    const final = flow === 'forward' ? leg.forwardBps : leg.reverseBps
    const live = lastBps(flow === 'forward' ? forwardSamples : reverseSamples)
    const active = measuring === flow
    const shown = active ? (live ?? final) : (final ?? (running ? live : null))
    const route =
      flow === 'forward'
        ? `${endpointLabel(leg.a)} → ${endpointLabel(leg.b)}`
        : `${endpointLabel(leg.b)} → ${endpointLabel(leg.a)}`
    return {
      flow,
      label: flow === 'forward' ? `↓ Ida · ${route}` : `↑ Volta · ${route}`,
      route,
      icon: flow === 'forward' ? 'mdi-arrow-right-bold' : 'mdi-arrow-left-bold',
      value:
        shown !== null ? compactBps(shown) : active ? 'medindo…' : running ? 'aguardando' : '—',
      bps: shown,
      active,
      // Sem a velocidade da porta, a comparação só repetiria o número grande.
      comparison: !running && final !== null && speedBps ? resultVsSpeed(final, speedBps) : null,
    }
  })
}
