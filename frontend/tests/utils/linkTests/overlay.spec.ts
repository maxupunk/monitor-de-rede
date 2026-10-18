import { describe, expect, it } from 'vitest'
import {
  canvasFlow,
  legOverlayLabel,
  overlayForPlan,
  overlayForTest,
  overlayState,
  pathOf,
} from '@/utils/linkTests/overlay'
import { endpoint, leg, plan, stage, test } from './fixtures'

describe('sobreposição do teste no mapa', () => {
  it('estado da perna vira estado da camada', () => {
    expect(overlayState('pending')).toBe('pending')
    expect(overlayState('running')).toBe('running')
    expect(overlayState('succeeded')).toBe('succeeded')
    expect(overlayState('failed')).toBe('failed')
    expect(overlayState('skipped')).toBe('skipped')
    expect(overlayState('cancelled')).toBe('skipped')
  })

  it('pendente → em teste (com o sentido) → resultado com Mbps', () => {
    const running = leg(1, {
      status: 'running',
      stages: [stage('serve', { status: 'succeeded' }), stage('forward', { status: 'running' })],
    })
    const done = leg(2, { status: 'succeeded', forwardBps: 487e6, reverseBps: 471e6 })
    const pending = leg(3)
    const live = (seq: number, flow: string) => (seq === 1 && flow === 'forward' ? 300e6 : null)
    const result = overlayForTest(test(1, { legs: [running, done, pending] }), live)

    expect(result.edges[101]).toEqual({ state: 'running', label: '↓ 300 Mbps', flow: 'forward' })
    expect(result.edges[102]).toEqual({ state: 'succeeded', label: '↓ 487 Mbps ↑ 471 Mbps' })
    expect(result.edges[103]).toEqual({ state: 'pending' })
    expect(result.highlight).toEqual({ nodeIds: [1, 2, 3, 4], edgeIds: [101, 102, 103] })
  })

  it('sem amostra ainda: "Preparando…"; falha e pulo com rótulo curto', () => {
    expect(legOverlayLabel(leg(1, { status: 'running' }))).toBe('Preparando…')
    expect(legOverlayLabel(leg(1, { status: 'failed' }))).toBe('Falhou')
    expect(legOverlayLabel(leg(1, { status: 'skipped' }))).toBe('Pulado')
    expect(legOverlayLabel(leg(1))).toBeUndefined()
  })

  it('inverte a animação quando a aresta do mapa foi desenhada de B para A', () => {
    const reverse = leg(1, {
      status: 'running',
      stages: [stage('reverse', { status: 'running' })],
    })
    expect(canvasFlow(reverse)).toBe('reverse')
    // Aresta 101 nasce em B (device 2): a volta B→A anda "para a frente" no canvas.
    expect(canvasFlow(reverse, (id) => (id === 101 ? 2 : null))).toBe('forward')
    expect(canvasFlow(leg(1, { status: 'running' }))).toBe('both')
  })

  it('perna através de switch sem IP: todas as arestas, rótulo só na primeira', () => {
    // AP-terraço (1) — switch-assistencia (5) — BORDA (2): arestas 11 e 12.
    const through = leg(1, {
      a: endpoint(1, 'AP-terraço'),
      b: endpoint(2, 'BORDA'),
      edgeId: null,
      edgeIds: [11, 12],
      viaDeviceIds: [5],
      viaNames: ['switch-assistencia'],
      status: 'running',
      stages: [stage('forward', { status: 'running' })],
    })
    // A aresta 12 foi desenhada de BORDA para o switch: a ida anda ao contrário nela.
    const source = (id: number) => (id === 11 ? 1 : id === 12 ? 2 : null)
    const live = () => 480e6
    const result = overlayForTest(test(1, { legs: [through] }), live, source)
    expect(result.edges[11]).toEqual({
      state: 'running',
      label: '↓ 480 Mbps ↑ 480 Mbps',
      flow: 'forward',
    })
    expect(result.edges[12]).toEqual({ state: 'running', flow: 'reverse' })
    expect(result.highlight).toEqual({ nodeIds: [1, 5, 2], edgeIds: [11, 12] })

    const done = { ...through, status: 'succeeded' as const, forwardBps: 9e8, reverseBps: 8e8 }
    const final = overlayForTest(test(1, { legs: [done] }), () => null, source)
    expect(final.edges[11]).toEqual({ state: 'succeeded', label: '↓ 900 Mbps ↑ 800 Mbps' })
    expect(final.edges[12]).toEqual({ state: 'succeeded' })

    const p = plan({
      legs: [{ ...plan().legs[0], edgeId: null, edgeIds: [11, 12], viaDeviceIds: [5] }],
    })
    expect(overlayForPlan(p).edges).toEqual({ 11: { state: 'pending' }, 12: { state: 'pending' } })
  })

  it('perna ponta a ponta não tem aresta: marca só as pontas', () => {
    const e2e = leg(1, {
      kind: 'end_to_end',
      edgeId: null,
      a: endpoint(50, 'NetMonitor', { kind: 'central' }),
      b: endpoint(9, 'Torre'),
      status: 'running',
    })
    const result = overlayForTest(test(1, { legs: [e2e] }))
    expect(result.edges).toEqual({})
    expect(result.nodes[50]).toMatchObject({ state: 'running' })
    expect(result.nodes[9]).toMatchObject({ state: 'running' })
    expect(result.highlight).toEqual({ nodeIds: [50, 9], edgeIds: [] })
  })

  it('central sem dispositivo não entra no destaque', () => {
    expect(
      pathOf([
        { a: endpoint(null, 'NetMonitor', { kind: 'central' }), b: endpoint(9, 'T'), edgeId: null },
      ])
    ).toEqual({ nodeIds: [9], edgeIds: [] })
  })

  it('plano: caminho destacado, enlaces pendentes e os que não rodam marcados', () => {
    const p = plan()
    p.legs.push({ ...p.legs[0], seq: 2, edgeId: 102, edgeIds: [102], runnable: false })
    expect(overlayForPlan(p).edges).toEqual({
      101: { state: 'pending' },
      102: { state: 'skipped' },
    })
  })
})
