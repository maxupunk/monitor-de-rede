/**
 * Apresentação do diagnóstico rápido por ping (ADR 015 §9). Regra pura: o
 * backend decide o nível de fila e se a capacidade sai; aqui só se escolhe
 * rótulo, cor e ícone.
 */
import type { CapacityConfidence } from '@/bindings/CapacityConfidence'
import type { QueueingLevel } from '@/bindings/QueueingLevel'
import type { QuickPingCapacity } from '@/bindings/QuickPingCapacity'
import type { QuickPingReport } from '@/bindings/QuickPingReport'
import type { TopologyNode } from '@/stores/topology'
import { formatBps, formatPercent } from '@/utils/formatters'
import type { ThemeColorToken } from '@/utils/topology/presentation'

export interface Badge {
  label: string
  color: ThemeColorToken
  icon: string
}

export const QUICK_PING_TITLE = 'Diagnóstico rápido (ping)'

export const QUICK_PING_SCOPE =
  'Mede o caminho todo da central até o equipamento, não um enlace; não substitui o teste de velocidade.'

const QUEUEING: Record<QueueingLevel, Badge> = {
  ok: { label: 'Sem sinal de fila', color: 'success', icon: 'mdi-check-circle' },
  moderado: { label: 'Fila moderada', color: 'warning', icon: 'mdi-alert' },
  alto: { label: 'Fila alta — possível saturação', color: 'error', icon: 'mdi-alert-octagon' },
}

export function queueingBadge(level: QueueingLevel): Badge {
  return QUEUEING[level]
}

const CONFIDENCE: Record<CapacityConfidence, string> = {
  baixa: 'Confiança baixa',
  muito_baixa: 'Confiança muito baixa',
}

export function confidenceLabel(confidence: CapacityConfidence): string {
  return CONFIDENCE[confidence]
}

/** Perda: as mesmas faixas do backend (`LOSS_MODERATE_PCT` / `LOSS_HIGH_PCT`). */
export function lossColor(lossPct: number): ThemeColorToken {
  if (lossPct >= 5) return 'error'
  if (lossPct >= 1) return 'warning'
  return 'success'
}

export function lossLabel(lossPct: number): string {
  return formatPercent(lossPct, lossPct > 0 && lossPct < 10 ? 1 : 0)
}

/** "82 Mbps – 170 Mbps": a faixa vem em Mbit/s. */
export function capacityRangeLabel(capacity: QuickPingCapacity): string {
  return `${formatBps(capacity.lowMbps * 1e6, { fractionDigits: 1 })} – ${formatBps(
    capacity.highMbps * 1e6,
    { fractionDigits: 1 }
  )}`
}

/** O que o diagnóstico pôde dizer da capacidade. */
export type CapacityOutcome = 'range' | 'not_estimated' | 'no_reply'

export function capacityOutcome(report: QuickPingReport): CapacityOutcome {
  if (report.capacity) return 'range'
  return report.samples === 0 ? 'no_reply' : 'not_estimated'
}

/** O diagnóstico só precisa de um IP — vale até para equipamento desconhecido. */
export function canQuickPing(node: Pick<TopologyNode, 'ipAddress'>): boolean {
  return Boolean(node.ipAddress?.trim())
}
