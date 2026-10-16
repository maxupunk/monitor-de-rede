import { describe, expect, it } from 'vitest'
import {
  bottleneckExplanation,
  bottleneckLeg,
  legCapacityBps,
  resultVsSpeed,
} from '@/utils/linkTests/results'
import { endpoint, leg, test } from './fixtures'

describe('resultado do teste de enlace', () => {
  it('compara o medido com a velocidade da porta', () => {
    expect(resultVsSpeed(487_000_000, 1_000_000_000)).toEqual({
      text: '487 Mbps de 1 Gbps — 49%',
      ratio: 0.487,
      color: 'warning',
    })
    expect(resultVsSpeed(940_000_000, 1_000_000_000)?.color).toBe('success')
    expect(resultVsSpeed(90_000_000, 1_000_000_000)?.color).toBe('error')
    expect(resultVsSpeed(90_000_000, null)).toEqual({ text: '90 Mbps', ratio: null, color: 'info' })
    expect(resultVsSpeed(null, 1e9)).toBeNull()
  })

  it('capacidade da perna é o pior sentido', () => {
    expect(legCapacityBps(leg(1, { forwardBps: 90e6, reverseBps: 80e6 }))).toBe(80e6)
    expect(legCapacityBps(leg(1, { forwardBps: 90e6 }))).toBe(90e6)
    expect(legCapacityBps(leg(1))).toBeNull()
  })

  it('gargalo: o do resumo do backend ou o menor enlace medido', () => {
    const legs = [
      leg(1, { status: 'succeeded', forwardBps: 900e6, reverseBps: 880e6 }),
      leg(2, { status: 'succeeded', forwardBps: 87e6, reverseBps: 85e6 }),
      leg(3, { status: 'succeeded', forwardBps: 400e6, reverseBps: 410e6 }),
    ]
    expect(bottleneckLeg({ legs, summary: null })?.seq).toBe(2)
    expect(
      bottleneckLeg({
        legs,
        summary: { bottleneckLegSeq: 3, minForwardBps: null, minReverseBps: null },
      })?.seq
    ).toBe(3)
  })

  it('explica o gargalo do caminho com a velocidade da porta', () => {
    const t = test(1, {
      kind: 'path',
      status: 'succeeded',
      legs: [
        leg(1, { status: 'succeeded', forwardBps: 900e6, reverseBps: 880e6 }),
        leg(2, { status: 'succeeded', forwardBps: 87e6, reverseBps: 85e6, edgeId: 102 }),
      ],
    })
    const text = bottleneckExplanation(t, (edgeId) => (edgeId === 102 ? 1e9 : null))
    expect(text).toContain('O gargalo é Dev-2 → Dev-3')
    expect(text).toContain('87 Mbps na ida e 85 Mbps na volta')
    expect(text).toContain('bem abaixo dos 1 Gbps da porta')
  })

  it('enlace único: quanto da porta passou', () => {
    const t = test(1, {
      status: 'succeeded',
      legs: [leg(1, { status: 'succeeded', forwardBps: 940e6, reverseBps: 930e6 })],
    })
    expect(bottleneckExplanation(t, () => 1e9)).toContain('o enlace entrega o que promete')
  })

  it('servidor: compara o ponta a ponta com o enlace mais lento', () => {
    const t = test(1, {
      kind: 'server',
      status: 'succeeded',
      legs: [
        leg(1, {
          kind: 'end_to_end',
          edgeId: null,
          a: endpoint(null, 'NetMonitor', { kind: 'central' }),
          status: 'succeeded',
          forwardBps: 86e6,
          reverseBps: 84e6,
        }),
        leg(2, { status: 'succeeded', forwardBps: 900e6, reverseBps: 900e6 }),
        leg(3, { status: 'succeeded', forwardBps: 88e6, reverseBps: 86e6 }),
      ],
    })
    expect(bottleneckExplanation(t)).toContain('bate com ele: é este enlace que limita')
  })

  it('sem medição não há explicação', () => {
    expect(bottleneckExplanation(test(1, { status: 'failed' }))).toBeNull()
  })
})
