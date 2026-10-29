import { describe, expect, it } from 'vitest'
import {
  CHART_PALETTE,
  areaPath,
  axisTicks,
  linePath,
  nearestIndex,
  niceCeiling,
  paletteColor,
  xAt,
  yAt,
  yDomain,
  type PlotArea,
} from '@/utils/charts/scale'

const area: PlotArea = { left: 10, top: 0, width: 100, height: 50 }

describe('núcleo de escala dos gráficos', () => {
  it('arredonda o teto para 1, 2, 2,5 ou 5 × 10ⁿ (ou 1024ⁿ)', () => {
    expect(niceCeiling(0)).toBe(1)
    expect(niceCeiling(-3)).toBe(1)
    expect(niceCeiling(7)).toBe(10)
    expect(niceCeiling(26)).toBe(50)
    expect(niceCeiling(2400)).toBe(2500)
    expect(niceCeiling(5.52 * 1024 ** 3, 1024)).toBe(10 * 1024 ** 3)
  })

  it('domínio começa em zero, respeita o teto mínimo e o domínio fixo', () => {
    expect(yDomain([3, 7, 9])).toEqual({ min: 0, max: 10 })
    expect(yDomain([])).toEqual({ min: 0, max: 1 })
    expect(yDomain([12], { minCeiling: 100 })).toEqual({ min: 0, max: 100 })
    expect(yDomain([-4, 8])).toEqual({ min: -5, max: 10 })
    expect(yDomain([NaN, 3])).toEqual({ min: 0, max: 5 })
    expect(yDomain([42], { fixed: { min: 0, max: 1 } })).toEqual({ min: 0, max: 1 })
  })

  it('marcas do eixo igualmente espaçadas, inclusive nas pontas', () => {
    expect(axisTicks(0, 100, 4)).toEqual([0, 25, 50, 75, 100])
    expect(axisTicks(5, 5, 4)).toEqual([5])
  })

  it('posiciona em pixels e acha a amostra mais próxima', () => {
    expect(xAt(0, 5, area)).toBe(10)
    expect(xAt(4, 5, area)).toBe(110)
    expect(xAt(0, 1, area)).toBe(60)
    expect(yAt(0, { min: 0, max: 10 }, area)).toBe(50)
    expect(yAt(10, { min: 0, max: 10 }, area)).toBe(0)
    // Fora do domínio fica preso à borda.
    expect(yAt(20, { min: 0, max: 10 }, area)).toBe(0)
    expect(nearestIndex(10, 5, area)).toBe(0)
    expect(nearestIndex(62, 5, area)).toBe(2)
    expect(nearestIndex(500, 5, area)).toBe(4)
    expect(nearestIndex(-50, 5, area)).toBe(0)
  })

  it('quebra a linha onde falta amostra e desenha degraus', () => {
    const points = [
      { x: 0, y: 10 },
      { x: 10, y: 20 },
      { x: 20, y: null },
      { x: 30, y: 5 },
    ]
    expect(linePath(points)).toBe('M0 10L10 20M30 5')
    expect(linePath(points.slice(0, 2), 'step')).toBe('M0 10H10V20')
    expect(linePath([])).toBe('')
  })

  it('área fecha cada trecho contínuo na base', () => {
    const points = [
      { x: 0, y: 10 },
      { x: 10, y: 20 },
      { x: 20, y: null },
      { x: 30, y: 5 },
    ]
    // O ponto isolado depois da falha não forma área.
    expect(areaPath(points, 50)).toBe('M0 50L0 10L10 20L10 50Z')
  })

  it('cor da série pela posição, sem sorteio', () => {
    expect(paletteColor(0)).toBe(CHART_PALETTE[0])
    expect(paletteColor(CHART_PALETTE.length)).toBe(CHART_PALETTE[0])
  })
})
