import { describe, expect, it } from 'vitest'
import type { LinkTestHealthSample, LinkTestSample } from '@/types/linkTests'
import {
  HEALTH_COLORS,
  cpuVerdict,
  healthSeries,
  testDeviceNames,
  testHealthLabels,
  trafficPoints,
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
  cpu: number | null,
  memory: number | null = null
): LinkTestHealthSample => ({ deviceId, atMs, cpu, memory })

describe('vazão no relógio da perna', () => {
  it('usa o atMs comum quando a perna o tem', () => {
    const { forward, reverse } = trafficPoints(
      [sample('forward', 2e8, 1000, 1200), sample('forward', 2.5e8, 2000, 2200)],
      [sample('reverse', 3e8, 1000, 6400)]
    )
    expect(forward).toEqual([
      { t: 1200, v: 2e8 },
      { t: 2200, v: 2.5e8 },
    ])
    expect(reverse).toEqual([{ t: 6400, v: 3e8 }])
  })

  it('perna antiga (sem atMs): a volta vem logo depois da ida', () => {
    const { forward, reverse } = trafficPoints(
      [sample('forward', 1, 1000), sample('forward', 1, 2000)],
      [sample('reverse', 1, 1000)]
    )
    expect(forward.map((p) => p.t)).toEqual([1000, 2000])
    expect(reverse.map((p) => p.t)).toEqual([4000])
  })
})

describe('CPU/RAM por equipamento', () => {
  const samples = [
    health(3, 1000, 40, 66),
    health(4, 1000, 10, 7),
    health(3, 2000, 97, 68),
    health(4, 2000, null, 8),
  ]

  it('uma série por equipamento, na ordem do caminho, com pico e última memória', () => {
    const series = healthSeries(samples, { 3: 'AP-Terraço', 4: 'AP-assistencia' }, [4, 3])
    expect(series.map((item) => item.name)).toEqual(['AP-assistencia', 'AP-Terraço'])
    expect(series[0].color).toBe(HEALTH_COLORS[0])
    expect(series[1].peakCpu).toBe(97)
    expect(series[1].lastMemory).toBe(68)
    // Leitura sem CPU não vira ponto zerado.
    expect(series[0].cpu).toEqual([{ t: 1000, v: 10 }])
  })

  it('valor do instante mais perto, só dentro da tolerância', () => {
    const points = [
      { t: 1000, v: 1 },
      { t: 2000, v: 2 },
    ]
    expect(valueAt(points, 1900)).toBe(2)
    expect(valueAt(points, 9000)).toBeNull()
  })

  it('rótulo do mapa: ao vivo a última leitura, no fim o pico', () => {
    expect(testHealthLabels(samples, true)[3]).toBe('⚠ CPU 97% · RAM 68%')
    expect(testHealthLabels(samples, false)[3]).toBe('⚠ CPU máx 97% · RAM 68%')
    expect(testHealthLabels(samples, false)[4]).toBe('CPU máx 10% · RAM 8%')
  })

  it('diz se o processador limitou o teste', () => {
    const names = { 3: 'AP-Terraço', 4: 'AP-assistencia' }
    expect(cpuVerdict(healthSeries(samples, names))).toBe(
      'CPU do AP-Terraço chegou a 97% durante a medição — o processador é o provável limite.'
    )
    expect(cpuVerdict(healthSeries([health(4, 1000, 12)], names))).toBe(
      'CPU do AP-assistencia ficou em no máximo 12% — não é o processador que limita.'
    )
    // Meio-termo ou sem leitura de CPU: nada a afirmar.
    expect(cpuVerdict(healthSeries([health(4, 1000, 75)], names))).toBeNull()
    expect(cpuVerdict(healthSeries([health(4, 1000, null, 50)], names))).toBeNull()
  })

  it('nomes vêm das pontas e dos equipamentos atravessados', () => {
    const names = testDeviceNames(
      test(1, { legs: [leg(1, { viaDeviceIds: [9], viaNames: ['BORDA'] })] })
    )
    expect(names).toEqual({ 1: 'Dev-1', 2: 'Dev-2', 9: 'BORDA' })
  })
})
