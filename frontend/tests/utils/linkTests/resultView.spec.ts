import { describe, expect, it } from 'vitest'
import { missedLegs, perHopRequest, repeatRequest } from '@/utils/linkTests/resultView'
import { legGauges } from '@/utils/linkTests/gauges'
import { endpoint, leg, stage, test } from './fixtures'

describe('resultado do teste, resumo primeiro', () => {
  it('o que não rodou vem com o primeiro motivo', () => {
    const t = test(1, {
      status: 'partial',
      legs: [
        leg(1, { status: 'succeeded' }),
        leg(2, {
          status: 'skipped',
          issues: ['RB2011 precisa do plugin «MikroTik RouterOS»', 'sem acesso SSH'],
        }),
      ],
    })
    expect(missedLegs(t)).toEqual([
      {
        seq: 2,
        title: 'Dev-2 → Dev-3',
        reason: 'RB2011 precisa do plugin «MikroTik RouterOS»',
        failed: false,
      },
    ])
  })

  it('"Medir enlace por enlace" só com outro equipamento com IP no meio', () => {
    const endToEnd = (via: number[]) =>
      test(2, {
        kind: 'end_to_end',
        legs: [leg(1, { kind: 'end_to_end', edgeId: null, viaDeviceIds: via, viaNames: [] })],
      })
    const hasIp = (id: number) => id === 5
    expect(perHopRequest(endToEnd([2, 5]), hasIp)).toEqual({
      kind: 'path',
      sourceDeviceId: 1,
      targetDeviceId: 2,
      spec: endToEnd([]).spec,
    })
    // Só switches sem IP no meio: enlace por enlace seria a mesma perna.
    expect(perHopRequest(endToEnd([2]), hasIp)).toBeNull()
    // Teste de enlace já é um enlace só.
    expect(perHopRequest(test(3), hasIp)).toBeNull()
    // Do servidor, a origem é o próprio servidor no mapa.
    const server = test(4, {
      kind: 'server',
      legs: [
        leg(1, {
          kind: 'end_to_end',
          a: endpoint(9, 'NetMonitor', { kind: 'central' }),
          viaDeviceIds: [5],
        }),
      ],
    })
    expect(perHopRequest(server, hasIp)).toMatchObject({ kind: 'path', sourceDeviceId: 9 })
  })

  it('"Testar de novo" refaz o mesmo pedido', () => {
    expect(repeatRequest(test(5))).toEqual({
      kind: 'link',
      linkId: 101,
      sourceDeviceId: 1,
      targetDeviceId: 2,
      spec: test(5).spec,
    })
    expect(repeatRequest(test(6, { kind: 'server', linkId: null, sourceDeviceId: null }))).toEqual({
      kind: 'server',
      targetDeviceId: 2,
      spec: test(6).spec,
    })
  })

  it('medidores: valor ao vivo do sentido medido, final depois', () => {
    const running = leg(1, {
      status: 'running',
      stages: [stage('forward', { status: 'running' }), stage('reverse')],
    })
    const sample = { tMs: 1000, bps: 487e6 }
    const [ida, volta] = legGauges(running, 'both', [sample], [], null)
    expect(ida).toMatchObject({ value: '487 Mbps', active: true, route: 'Dev-1 → Dev-2' })
    expect(volta).toMatchObject({ value: 'aguardando', active: false })
    const done = leg(1, { status: 'succeeded', forwardBps: 900e6, reverseBps: 400e6 })
    const gauges = legGauges(done, 'forward', [], [], 1e9)
    expect(gauges).toHaveLength(1)
    expect(gauges[0].comparison?.ratio).toBeCloseTo(0.9)
  })
})
