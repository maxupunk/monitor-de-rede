import { describe, expect, it } from 'vitest'
import type { LinkTestHealthSample, LinkTestSample } from '@/types/linkTests'
import {
  cpuVerdict,
  deviceHealth,
  measuredSteps,
  relativeTime,
  stepValueAt,
  testDeviceNames,
  testHealthLabels,
  testWindow,
  trafficPoints,
  usageStory,
  valueAt,
} from '@/utils/linkTests/health'
import { leg, test } from './fixtures'

const sample = (
  flow: LinkTestSample['flow'],
  bps: number,
  tMs: number,
  atMs?: number
): LinkTestSample => ({ flow, bps, tMs, ...(atMs === undefined ? {} : { atMs }) })

const health = (
  deviceId: number,
  atMs: number,
  phase: LinkTestHealthSample['phase'],
  cpu: number | null,
  memory: number | null = null
): LinkTestHealthSample => ({ deviceId, atMs, phase, cpu, memory })

const NAMES = { 3: 'AP-Terraço', 4: 'AP-assistencia' }

describe('vazão no relógio da perna', () => {
  it('usa o atMs comum quando a perna o tem', () => {
    const { forward, reverse } = trafficPoints(
      [sample('forward', 2e8, 1000, 9200), sample('forward', 2.5e8, 2000, 10200)],
      [sample('reverse', 3e8, 1000, 15400)]
    )
    expect(forward.map((p) => p.t)).toEqual([9200, 10200])
    expect(reverse).toEqual([{ t: 15400, v: 3e8 }])
  })

  it('perna antiga (sem atMs): a volta vem logo depois da ida', () => {
    const { forward, reverse } = trafficPoints(
      [sample('forward', 1, 1000), sample('forward', 1, 2000)],
      [sample('reverse', 1, 1000)]
    )
    expect(forward.map((p) => p.t)).toEqual([1000, 2000])
    expect(reverse.map((p) => p.t)).toEqual([4000])
  })

  it('janela do teste: do primeiro segundo de tráfego ao último', () => {
    expect(testWindow([{ t: 9200, v: 1 }], [{ t: 15400, v: 1 }], [])).toEqual({
      start: 8200,
      end: 15400,
    })
    expect(testWindow([], [], [])).toBeNull()
  })

  it('tempo relativo ao começo do teste', () => {
    expect(relativeTime(-8000)).toBe('−8 s')
    expect(relativeTime(0)).toBe('0 s')
    expect(relativeTime(5500)).toBe('5,5 s')
  })
})

describe('antes, durante e depois de cada equipamento', () => {
  const samples = [
    health(3, 0, 'before', 12, 74),
    health(3, 4000, 'before', 10, 74),
    health(3, 9000, 'during', 75, 80),
    health(3, 12000, 'during', 96, 86),
    health(3, 20000, 'after', 14, 76),
    health(4, 0, 'before', 2, 7),
    health(4, 12000, 'during', 3, 7),
  ]

  it('antes = última leitura sem tráfego; pico no teste; depois = última', () => {
    const [terraco, assist] = deviceHealth(samples, NAMES, [3, 4])
    expect(terraco.before).toEqual({ cpu: 10, memory: 74 })
    expect(terraco.peak).toEqual({ cpu: 96, memory: 86 })
    expect(terraco.after).toEqual({ cpu: 14, memory: 76 })
    expect(assist.name).toBe('AP-assistencia')
  })

  it('resumo de uma faixa: a mudança de CPU e a de RAM só quando anda', () => {
    const [terraco, assist] = deviceHealth(samples, NAMES, [3, 4])
    expect(usageStory(terraco)).toBe('CPU 10% → 96% no teste, 14% depois · RAM 74% → 86%')
    expect(usageStory(assist)).toBe('CPU 2% → 3% no teste · RAM 7%')
  })

  it('frase do resumo: foi o teste que levou a CPU ao teto', () => {
    expect(cpuVerdict(deviceHealth(samples, NAMES))).toBe(
      'A CPU do AP-Terraço subiu de 10% para 96% no teste (voltou a 14% depois) — o processador é o provável limite.'
    )
  })

  it('frase do resumo: o equipamento já estava ocupado antes', () => {
    const busy = [health(3, 0, 'before', 88), health(3, 9000, 'during', 97)]
    expect(cpuVerdict(deviceHealth(busy, NAMES))).toBe(
      'O AP-Terraço já estava com a CPU em 88% antes do teste e chegou a 97% — ele já estava ocupado antes de medir.'
    )
  })

  it('frase do resumo: CPU folgada não é o limite; sem leitura no teste, nada', () => {
    const relaxed = [health(4, 0, 'before', 2), health(4, 9000, 'during', 20)]
    expect(cpuVerdict(deviceHealth(relaxed, NAMES))).toBe(
      'A CPU ficou folgada (AP-assistencia: de 2% para 20% no teste) — não é o processador que limita.'
    )
    expect(cpuVerdict(deviceHealth([health(4, 0, 'before', 2)], NAMES))).toBeNull()
  })

  it('rótulo do mapa: ao vivo a leitura mais recente; no fim, antes → pico', () => {
    expect(testHealthLabels(samples, true)[3]).toBe('CPU 14% · RAM 76%')
    expect(testHealthLabels(samples, false)[3]).toBe('⚠ CPU 10→96% · RAM 86%')
    expect(testHealthLabels(samples, false)[4]).toBe('CPU 2→3% · RAM 7%')
  })

  it('degraus de contador: o valor cai no trecho que ele mediu', () => {
    // Contador renovado a cada 5 s: 11 em 0–4, 72 a partir de 5, 96 a partir de 10.
    const points = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((s) => ({
      t: s * 1000,
      v: s < 5 ? 11 : s < 10 ? 72 : 96,
    }))
    const steps = measuredSteps(points)
    expect(steps).toEqual([
      { from: 0, to: 5000, v: 72 },
      { from: 5000, to: 10000, v: 96 },
      { from: 10000, to: 11000, v: 96 },
    ])
    expect(stepValueAt(steps, 7000)).toBe(96)
    expect(stepValueAt(steps, 99000)).toBeNull()
    // Leitura constante: um trecho só.
    expect(
      measuredSteps([
        { t: 0, v: 5 },
        { t: 3000, v: 5 },
      ])
    ).toEqual([{ from: 0, to: 3000, v: 5 }])
  })

  it('valor do instante mais perto, só dentro da tolerância', () => {
    const points = [
      { t: 1000, v: 1 },
      { t: 2000, v: 2 },
    ]
    expect(valueAt(points, 1900)).toBe(2)
    expect(valueAt(points, 9000)).toBeNull()
  })

  it('nomes vêm das pontas e dos equipamentos atravessados', () => {
    const names = testDeviceNames(
      test(1, { legs: [leg(1, { viaDeviceIds: [9], viaNames: ['BORDA'] })] })
    )
    expect(names).toEqual({ 1: 'Dev-1', 2: 'Dev-2', 9: 'BORDA' })
  })
})
