/**
 * Leitura do resultado de um teste de enlace: comparação com a velocidade da
 * porta, gargalo do caminho e a explicação em português para quem opera.
 */
import { formatLinkSpeed, formatPercent } from '@/utils/formatters'
import type { ThemeColorToken } from '@/utils/topology/presentation'
import type { LinkTest, LinkTestLeg } from '@/types/linkTests'
import { compactBps, legTitle } from './presentation'

export interface SpeedComparison {
  /** "487 Mbps de 1 Gbps — 49%" (ou só "487 Mbps" sem velocidade conhecida). */
  text: string
  /** 0..1 (pode passar de 1 se a porta informar menos do que passou). */
  ratio: number | null
  color: ThemeColorToken
}

/** A partir de quanto da porta o enlace é considerado saudável. */
export const HEALTHY_RATIO = 0.7
export const DEGRADED_RATIO = 0.4

export function ratioColor(ratio: number | null): ThemeColorToken {
  if (ratio === null) return 'info'
  if (ratio >= HEALTHY_RATIO) return 'success'
  if (ratio >= DEGRADED_RATIO) return 'warning'
  return 'error'
}

/** Resultado medido contra a velocidade nominal do enlace. */
export function resultVsSpeed(
  bps: number | null | undefined,
  speedBps: number | null | undefined
): SpeedComparison | null {
  if (typeof bps !== 'number' || !Number.isFinite(bps)) return null
  if (!speedBps || speedBps <= 0) return { text: compactBps(bps), ratio: null, color: 'info' }
  const ratio = bps / speedBps
  return {
    text: `${compactBps(bps)} de ${formatLinkSpeed(speedBps)} — ${formatPercent(ratio * 100, 0)}`,
    ratio,
    color: ratioColor(ratio),
  }
}

/** O pior dos dois sentidos medidos (o que limita a perna). */
export function legCapacityBps(leg: Pick<LinkTestLeg, 'forwardBps' | 'reverseBps'>): number | null {
  const values = [leg.forwardBps, leg.reverseBps].filter(
    (value): value is number => typeof value === 'number' && Number.isFinite(value)
  )
  return values.length > 0 ? Math.min(...values) : null
}

/**
 * Perna gargalo: a que o backend apontou no resumo ou, sem resumo, a de
 * menor capacidade entre as pernas de enlace medidas.
 */
export function bottleneckLeg(test: Pick<LinkTest, 'legs' | 'summary'>): LinkTestLeg | null {
  const seq = test.summary?.bottleneckLegSeq
  if (seq !== null && seq !== undefined) {
    return test.legs.find((leg) => leg.seq === seq) ?? null
  }
  let worst: LinkTestLeg | null = null
  let worstBps = Infinity
  for (const leg of test.legs) {
    if (leg.kind !== 'link') continue
    const capacity = legCapacityBps(leg)
    if (capacity !== null && capacity < worstBps) {
      worst = leg
      worstBps = capacity
    }
  }
  return worst
}

function directionsText(leg: Pick<LinkTestLeg, 'forwardBps' | 'reverseBps'>): string {
  const parts: string[] = []
  if (typeof leg.forwardBps === 'number') parts.push(`${compactBps(leg.forwardBps)} na ida`)
  if (typeof leg.reverseBps === 'number') parts.push(`${compactBps(leg.reverseBps)} na volta`)
  return parts.join(' e ')
}

/**
 * Explicação do resultado. Um enlace só: o quanto da porta passou. Caminho:
 * qual enlace limita e por quê. Servidor: o ponta a ponta comparado ao
 * enlace mais lento. `speedOf(edgeId)` dá a velocidade nominal conhecida.
 */
export function bottleneckExplanation(
  test: Pick<LinkTest, 'kind' | 'legs' | 'summary' | 'status'>,
  speedOf: (edgeId: number) => number | null = () => null
): string | null {
  const measured = test.legs.filter((leg) => legCapacityBps(leg) !== null)
  if (measured.length === 0) return null

  const worst = bottleneckLeg(test)
  const linkLegs = test.legs.filter((leg) => leg.kind === 'link')

  if (worst === null) {
    const endToEnd = test.legs.find((leg) => leg.kind === 'end_to_end')
    if (endToEnd && legCapacityBps(endToEnd) !== null) {
      return `Ponta a ponta passou ${directionsText(endToEnd)}.`
    }
    return null
  }

  const capacity = legCapacityBps(worst) ?? 0
  const speed = worst.edgeId !== null ? speedOf(worst.edgeId) : null
  const ratio = speed ? capacity / speed : null
  const portNote =
    ratio === null
      ? ''
      : ratio >= HEALTHY_RATIO
        ? ` — ${formatPercent(ratio * 100, 0)} dos ${formatLinkSpeed(speed)} da porta: o enlace entrega o que promete.`
        : ratio >= DEGRADED_RATIO
          ? ` — só ${formatPercent(ratio * 100, 0)} dos ${formatLinkSpeed(speed)} da porta: vale olhar uso, CPU dos equipamentos ou sinal do rádio.`
          : ` — bem abaixo dos ${formatLinkSpeed(speed)} da porta (${formatPercent(ratio * 100, 0)}): suspeite de rádio, duplex, CPU do equipamento ou perda.`

  if (linkLegs.length <= 1 && test.kind === 'link') {
    return `${legTitle(worst)} passou ${directionsText(worst)}${portNote || '.'}`
  }

  const base = `O gargalo é ${legTitle(worst)}: ${directionsText(worst)}${portNote || ', o menor valor do caminho.'}`
  const endToEnd = test.legs.find((leg) => leg.kind === 'end_to_end')
  const e2e = endToEnd ? legCapacityBps(endToEnd) : null
  if (e2e !== null) {
    const close = capacity > 0 && Math.abs(e2e - capacity) / capacity <= 0.15
    return `${base} ${
      close
        ? `O ponta a ponta (${compactBps(e2e)}) bate com ele: é este enlace que limita.`
        : `O ponta a ponta mediu ${compactBps(e2e)} — a diferença vem do roteamento ou da carga no momento.`
    }`
  }
  return base
}
