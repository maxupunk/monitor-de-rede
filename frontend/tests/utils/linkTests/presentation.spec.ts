import { describe, expect, it } from 'vitest'
import {
  compactBps,
  endpointLabel,
  flowPairLabel,
  legTitle,
  requestTargetLabel,
  serverSideLabel,
  stageEndpoint,
  stepStatusPresentation,
  testStatusPresentation,
  testTitle,
} from '@/utils/linkTests/presentation'
import { endpoint, leg, test } from './fixtures'

describe('apresentação do teste de enlace', () => {
  it('estados com cor semântica (nunca cinza) e queda segura para valor desconhecido', () => {
    expect(testStatusPresentation('succeeded')).toMatchObject({
      label: 'Concluído',
      color: 'success',
    })
    expect(testStatusPresentation('partial').color).toBe('warning')
    expect(testStatusPresentation('failed').color).toBe('error')
    expect(testStatusPresentation('???').label).toBe('Na fila')
    expect(stepStatusPresentation('skipped')).toMatchObject({ label: 'Pulada', color: 'warning' })
  })

  it('título da perna e das pontas', () => {
    expect(legTitle(leg(1))).toBe('Dev-1 → Dev-2')
    expect(endpointLabel({ kind: 'central', name: '' })).toBe('Servidor NetMonitor')
    expect(serverSideLabel(leg(1))).toBe('servidor em Dev-2')
    expect(serverSideLabel(leg(1, { serverSide: null }))).toBeNull()
  })

  it('título do teste por tipo', () => {
    expect(testTitle(test(1))).toBe('Enlace · Dev-1 → Dev-2')
    expect(testTitle(test(2, { kind: 'path', legs: [leg(1), leg(2), leg(3)] }))).toBe(
      'Caminho completo · Dev-1 → Dev-4'
    )
    const server = test(3, {
      kind: 'server',
      legs: [
        leg(1, {
          kind: 'end_to_end',
          edgeId: null,
          a: endpoint(null, 'NetMonitor', { kind: 'central' }),
          b: endpoint(9, 'Torre'),
        }),
        leg(2, { b: endpoint(9, 'Torre') }),
      ],
    })
    expect(testTitle(server)).toBe('Do servidor até Torre')
    expect(testTitle(test(4, { legs: [] }))).toBe('Enlace · teste #4')
  })

  it('Mbps compactos com três algarismos e par de sentidos', () => {
    expect(compactBps(487_300_000)).toBe('487 Mbps')
    expect(compactBps(94_230_000)).toBe('94.2 Mbps')
    expect(compactBps(1_234_000_000)).toBe('1.2 Gbps')
    expect(compactBps(0)).toBe('0 bps')
    expect(compactBps(null)).toBe('—')
    expect(flowPairLabel(487_000_000, 471_000_000)).toBe('↓ 487 Mbps ↑ 471 Mbps')
    expect(flowPairLabel(null, 471_000_000)).toBe('↑ 471 Mbps')
    expect(flowPairLabel(null, null)).toBeNull()
  })

  it('cada etapa roda numa ponta: servidor e encerramento em quem serve, medição em quem dirige', () => {
    const l = leg(1, { serverSide: 'b' })
    expect(stageEndpoint(l, 'prepare_a').name).toBe('Dev-1')
    expect(stageEndpoint(l, 'serve').name).toBe('Dev-2')
    expect(stageEndpoint(l, 'stop').name).toBe('Dev-2')
    expect(stageEndpoint(l, 'forward').name).toBe('Dev-1')
  })

  it('alvo pedido (agendamento)', () => {
    const name = (id: number) => `D${id}`
    expect(requestTargetLabel({ kind: 'link', sourceDeviceId: 1, targetDeviceId: 2 }, name)).toBe(
      'D1 ↔ D2'
    )
    expect(
      requestTargetLabel({ kind: 'server', sourceDeviceId: null, targetDeviceId: 2 }, name)
    ).toBe('Servidor → D2')
    expect(
      requestTargetLabel({ kind: 'path', sourceDeviceId: 3, targetDeviceId: null }, name)
    ).toBe('D3 → ?')
  })
})
