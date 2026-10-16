/**
 * Parâmetros do teste de enlace (ADR 015 §5): padrões, limites, validação,
 * estimativa de duração e agendamento. Funções puras — o assistente, o
 * diálogo de agendamento e o painel ao vivo usam as mesmas regras.
 */
import { formatElapsedMs } from '@/utils/formatters'
import type {
  LinkTestDirection,
  LinkTestPlanLeg,
  LinkTestScheduleInput,
  LinkTestSpec,
  LinkTestSpecInput,
} from '@/types/linkTests'

export const DURATION_MIN_S = 1
export const DURATION_MAX_S = 60
export const DURATION_DEFAULT_S = 5
export const STREAMS_MIN = 1
export const STREAMS_MAX = 8
export const LIMIT_MAX_MBPS = 100_000

/**
 * Segundos de preparo e encerramento de cada perna (subir servidor, conectar,
 * parar) — só para a estimativa; o plano do backend é a fonte da verdade.
 */
export const LEG_OVERHEAD_S = 6

export const DEFAULT_SPEC: Readonly<LinkTestSpec> = Object.freeze({
  durationS: DURATION_DEFAULT_S,
  direction: 'both',
  streams: 1,
  limitMbps: null,
})

function clampInt(value: unknown, min: number, max: number, fallback: number): number {
  const n = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(n)) return fallback
  return Math.min(max, Math.max(min, Math.round(n)))
}

function isDirection(value: unknown): value is LinkTestDirection {
  return value === 'both' || value === 'forward' || value === 'reverse'
}

/** Completa e limita a especificação (o que o backend aplicaria). */
export function normalizeSpec(input?: LinkTestSpecInput | null): LinkTestSpec {
  const limit = input?.limitMbps
  return {
    durationS: clampInt(input?.durationS, DURATION_MIN_S, DURATION_MAX_S, DURATION_DEFAULT_S),
    direction: isDirection(input?.direction) ? input.direction : DEFAULT_SPEC.direction,
    streams: clampInt(input?.streams, STREAMS_MIN, STREAMS_MAX, DEFAULT_SPEC.streams),
    limitMbps:
      typeof limit === 'number' && Number.isFinite(limit) && limit > 0
        ? Math.min(LIMIT_MAX_MBPS, Math.round(limit))
        : null,
  }
}

/** Erros de preenchimento, em português, prontos para a tela. Vazio = válido. */
export function validateSpec(input: LinkTestSpecInput): string[] {
  const errors: string[] = []
  const { durationS, streams, limitMbps } = input
  if (
    durationS !== undefined &&
    (!Number.isInteger(durationS) || durationS < DURATION_MIN_S || durationS > DURATION_MAX_S)
  ) {
    errors.push(`A duração vai de ${DURATION_MIN_S} a ${DURATION_MAX_S} segundos por sentido.`)
  }
  if (
    streams !== undefined &&
    (!Number.isInteger(streams) || streams < STREAMS_MIN || streams > STREAMS_MAX)
  ) {
    errors.push(`Fluxos paralelos vão de ${STREAMS_MIN} a ${STREAMS_MAX}.`)
  }
  if (input.direction !== undefined && !isDirection(input.direction)) {
    errors.push('Escolha o sentido do teste.')
  }
  if (
    limitMbps !== undefined &&
    limitMbps !== null &&
    (!Number.isFinite(limitMbps) || limitMbps <= 0 || limitMbps > LIMIT_MAX_MBPS)
  ) {
    errors.push('O limite precisa ser um número de Mbps maior que zero (ou vazio, sem limite).')
  }
  return errors
}

export function directionCount(direction: LinkTestDirection): number {
  return direction === 'both' ? 2 : 1
}

/** Segundos esperados de uma perna: medição nos sentidos pedidos + preparo. */
export function legExpectedSeconds(
  leg: Pick<LinkTestPlanLeg, 'durationS' | 'runnable'>,
  direction: LinkTestDirection
): number {
  if (!leg.runnable) return 0
  return leg.durationS * directionCount(direction) + LEG_OVERHEAD_S
}

/** Estimativa local (antes do plano): pernas × (duração × sentidos + preparo). */
export function estimateSeconds(spec: LinkTestSpecInput, legCount = 1): number {
  const normalized = normalizeSpec(spec)
  return (
    Math.max(1, legCount) *
    (normalized.durationS * directionCount(normalized.direction) + LEG_OVERHEAD_S)
  )
}

/** "cerca de 25 s", "cerca de 1 min 10 s". */
export function estimatedDurationText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return 'alguns segundos'
  return `cerca de ${formatElapsedMs(Math.round(seconds) * 1000)}`
}

/** "5 s por sentido · ida e volta · 2 fluxos · até 100 Mbps". */
export function specSummary(input: LinkTestSpecInput): string {
  const spec = normalizeSpec(input)
  const parts = [
    `${spec.durationS} s por sentido`,
    spec.direction === 'both'
      ? 'ida e volta'
      : spec.direction === 'forward'
        ? 'só ida'
        : 'só volta',
  ]
  if (spec.streams > 1) parts.push(`${spec.streams} fluxos`)
  if (spec.limitMbps !== null) parts.push(`até ${spec.limitMbps} Mbps`)
  return parts.join(' · ')
}

// --- Agendamento -------------------------------------------------------------

export const MIN_INTERVAL_MINUTES = 15

export const INTERVAL_PRESETS: ReadonlyArray<{ value: number; title: string }> = [
  { value: 15, title: '15 min' },
  { value: 60, title: '1 h' },
  { value: 360, title: '6 h' },
  { value: 1440, title: '24 h' },
]

/** "a cada 15 min", "a cada 6 h", "a cada 1 dia", "a cada 90 min". */
export function intervalLabel(minutes: number): string {
  if (minutes % 1440 === 0) {
    const days = minutes / 1440
    return `a cada ${days} ${days === 1 ? 'dia' : 'dias'}`
  }
  if (minutes % 60 === 0) return `a cada ${minutes / 60} h`
  return `a cada ${minutes} min`
}

const HHMM = /^([01]\d|2[0-3]):[0-5]\d$/

/** "das 22:00 às 06:00" ou `null` (qualquer horário). */
export function windowLabel(start: string | null, end: string | null): string | null {
  if (!start || !end) return null
  return `das ${start} às ${end}`
}

export function validateSchedule(input: LinkTestScheduleInput): string[] {
  const errors: string[] = []
  if (!input.name.trim()) errors.push('Dê um nome ao agendamento.')
  if (!Number.isInteger(input.intervalMinutes) || input.intervalMinutes < MIN_INTERVAL_MINUTES) {
    errors.push(`O intervalo mínimo é de ${MIN_INTERVAL_MINUTES} minutos.`)
  }
  const hasStart = Boolean(input.windowStart)
  const hasEnd = Boolean(input.windowEnd)
  if (hasStart !== hasEnd) {
    errors.push('Preencha o início e o fim da janela de horário (ou deixe os dois vazios).')
  } else if (
    hasStart &&
    (!HHMM.test(input.windowStart ?? '') || !HHMM.test(input.windowEnd ?? ''))
  ) {
    errors.push('Use o formato HH:MM na janela de horário.')
  } else if (hasStart && input.windowStart === input.windowEnd) {
    errors.push('O início e o fim da janela não podem ser iguais.')
  }
  if (input.spec) errors.push(...validateSpec(input.spec))
  return errors
}
