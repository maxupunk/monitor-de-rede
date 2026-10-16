/**
 * Andamento de um teste: perna atual, tempo decorrido e o que falta. O
 * "agora" vem de quem chama (a última atividade recebida pelo SSE) — nada
 * aqui usa relógio próprio nem temporizador.
 */
import type { LinkTest, LinkTestLeg } from '@/types/linkTests'
import { legExpectedSeconds } from './spec'
import { isActiveStatus } from './presentation'

function toMs(value: string | null | undefined): number | null {
  if (!value) return null
  const ms = Date.parse(value)
  return Number.isFinite(ms) ? ms : null
}

/** A perna em execução ou, entre pernas, a próxima a rodar. */
export function currentLeg(test: Pick<LinkTest, 'legs'>): LinkTestLeg | null {
  return (
    test.legs.find((leg) => leg.status === 'running') ??
    test.legs.find((leg) => leg.status === 'pending') ??
    null
  )
}

export function legsDone(test: Pick<LinkTest, 'legs'>): number {
  return test.legs.filter((leg) => leg.status !== 'pending' && leg.status !== 'running').length
}

/** Decorrido do início até o fim (ou até `nowMs`, enquanto roda). */
export function testElapsedMs(
  test: Pick<LinkTest, 'startedAt' | 'finishedAt'>,
  nowMs: number
): number | null {
  const start = toMs(test.startedAt)
  if (start === null) return null
  const end = toMs(test.finishedAt) ?? nowMs
  return Math.max(0, end - start)
}

/** Quanto falta, pela duração esperada das pernas ainda não concluídas. */
export function testRemainingMs(
  test: Pick<LinkTest, 'status' | 'legs' | 'spec'>,
  nowMs: number
): number | null {
  if (!isActiveStatus(test.status)) return 0
  let remaining = 0
  for (const leg of test.legs) {
    const expected = legExpectedSeconds(leg, test.spec.direction) * 1000
    if (leg.status === 'pending') remaining += expected
    else if (leg.status === 'running') {
      const start = toMs(leg.startedAt)
      remaining += start === null ? expected : Math.max(0, expected - (nowMs - start))
    }
  }
  return remaining
}

/** 0..1 — pernas concluídas mais a fração da perna em andamento. */
export function testProgress(
  test: Pick<LinkTest, 'status' | 'legs' | 'spec'>,
  nowMs: number
): number {
  if (!isActiveStatus(test.status)) return 1
  const total = test.legs.reduce(
    (sum, leg) => sum + legExpectedSeconds(leg, test.spec.direction),
    0
  )
  if (total <= 0) return 0
  const remaining = (testRemainingMs(test, nowMs) ?? 0) / 1000
  return Math.max(0, Math.min(1, 1 - remaining / total))
}
