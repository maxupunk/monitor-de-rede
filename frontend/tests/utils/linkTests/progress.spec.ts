import { describe, expect, it } from 'vitest'
import {
  currentLeg,
  legsDone,
  testElapsedMs,
  testProgress,
  testRemainingMs,
} from '@/utils/linkTests/progress'
import { leg, test } from './fixtures'

const START = Date.parse('2026-10-16T12:00:01Z')

describe('andamento do teste', () => {
  const running = test(1, {
    legs: [
      leg(1, { status: 'succeeded' }),
      leg(2, { status: 'running', startedAt: '2026-10-16T12:00:17Z' }),
      leg(3),
    ],
  })

  it('perna atual e concluídas', () => {
    expect(currentLeg(running)?.seq).toBe(2)
    expect(legsDone(running)).toBe(1)
    expect(currentLeg(test(2, { legs: [leg(1, { status: 'succeeded' })] }))).toBeNull()
  })

  it('decorrido usa o "agora" de quem chama (sem relógio próprio)', () => {
    expect(testElapsedMs(running, START + 20_000)).toBe(20_000)
    expect(testElapsedMs({ startedAt: null, finishedAt: null }, START)).toBeNull()
    expect(
      testElapsedMs({ startedAt: '2026-10-16T12:00:01Z', finishedAt: '2026-10-16T12:00:31Z' }, 0)
    ).toBe(30_000)
  })

  it('falta: perna em andamento pelo tempo dela + pernas pendentes', () => {
    // Cada perna: 5 s × 2 sentidos + 6 s de preparo = 16 s.
    const now = Date.parse('2026-10-16T12:00:21Z')
    expect(testRemainingMs(running, now)).toBe(12_000 + 16_000)
    expect(testRemainingMs(test(3, { status: 'succeeded' }), now)).toBe(0)
    expect(testProgress(running, now)).toBeCloseTo(1 - 28 / 48)
    expect(testProgress(test(4, { status: 'failed' }), now)).toBe(1)
  })
})
