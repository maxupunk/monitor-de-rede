import { describe, expect, it } from 'vitest'
import {
  buildStackedUsage,
  niceCeiling,
  pickLayers,
  OTHERS_COLOR,
  OTHERS_ID,
  STACK_PALETTE,
  type UsageSample,
} from '@/utils/stackedUsage'

const labels = { a: 'api', b: 'banco', c: 'cache' }

describe('consumo empilhado', () => {
  it('empilha as faixas e a última termina no total', () => {
    const samples: UsageSample[] = [
      { time: 't1', values: { a: 10, b: 5 } },
      { time: 't2', values: { a: 20, b: 5, c: 1 } },
    ]
    const stack = buildStackedUsage(samples, labels)

    expect(stack.totals).toEqual([15, 26])
    expect(stack.peak).toBe(26)
    for (const layer of stack.layers) {
      layer.values.forEach((value, index) => {
        expect(layer.upper[index] - layer.lower[index]).toBeCloseTo(value)
      })
    }
    expect(stack.layers.at(-1)?.upper).toEqual(stack.totals)
    expect(stack.layers.find((layer) => layer.id === 'c')?.values).toEqual([0, 1])
  })

  it('a cor segue o container pela ordem em que apareceu, não pelo ranking', () => {
    const antes = buildStackedUsage([{ time: 't1', values: { a: 1, b: 50 } }], labels)
    const depois = buildStackedUsage(
      [
        { time: 't1', values: { a: 1, b: 50 } },
        { time: 't2', values: { a: 90, b: 1 } },
      ],
      labels
    )
    const cor = (stack: typeof antes, id: string) =>
      stack.layers.find((layer) => layer.id === id)?.color
    expect(cor(antes, 'a')).toBe(STACK_PALETTE[0])
    expect(cor(depois, 'a')).toBe(STACK_PALETTE[0])
    expect(cor(depois, 'b')).toBe(STACK_PALETTE[1])
  })

  it('o que passa do limite vira "Outros" em cinza, somado', () => {
    const values = { a: 30, b: 20, c: 1, d: 2 }
    const stack = buildStackedUsage([{ time: 't1', values }], labels, 2)
    const outros = stack.layers.find((layer) => layer.id === OTHERS_ID)

    expect(stack.layers.map((layer) => layer.id)).toEqual(['a', 'b', OTHERS_ID])
    expect(outros?.label).toBe('Outros (2)')
    expect(outros?.color).toBe(OTHERS_COLOR)
    expect(outros?.values).toEqual([3])

    const umSo = buildStackedUsage([{ time: 't1', values: { a: 3, b: 2, c: 1 } }], labels, 2)
    expect(umSo.layers.at(-1)?.label).toBe('cache')
  })

  it('janela vazia não quebra', () => {
    const stack = buildStackedUsage([], labels)
    expect(stack.layers).toEqual([])
    expect(stack.peak).toBe(0)
  })

  it('teto do eixo arredonda para 1, 2, 2,5 ou 5 × 10ⁿ, dentro da unidade binária para bytes', () => {
    expect(niceCeiling(0)).toBe(1)
    expect(niceCeiling(7)).toBe(10)
    expect(niceCeiling(26)).toBe(50)
    expect(niceCeiling(120)).toBe(200)
    expect(niceCeiling(2400)).toBe(2500)
    expect(niceCeiling(5.52 * 1024 ** 3, 1024)).toBe(10 * 1024 ** 3)
    expect(niceCeiling(700 * 1024 ** 2, 1024)).toBe(1000 * 1024 ** 2)
  })
})

describe('o que o tooltip mostra', () => {
  // Pilha de baixo para cima: a (0–50), b (50–51), c (51–52), d (52–100).
  const stack = buildStackedUsage([{ time: 't', values: { a: 50, b: 1, c: 1, d: 48 } }], {
    a: 'a',
    b: 'b',
    c: 'c',
    d: 'd',
  })

  it('sobre uma faixa larga, só ela', () => {
    expect(pickLayers(stack, 0, 20, 3)).toEqual({ hovered: 'a', rows: ['a'], hidden: 3 })
    expect(pickLayers(stack, 0, 80, 3)).toEqual({ hovered: 'd', rows: ['d'], hidden: 3 })
  })

  it('sobre faixas finas, as vizinhas próximas, de cima para baixo', () => {
    // b e c estão sob o cursor; a e d são largas e ficam longe dele.
    expect(pickLayers(stack, 0, 50.5, 3)).toEqual({ hovered: 'b', rows: ['c', 'b'], hidden: 2 })
    // Nunca mais que três, mesmo com tolerância grande.
    expect(pickLayers(stack, 0, 50.5, 40).rows).toHaveLength(3)
  })

  it('acima da pilha, os três maiores, de cima para baixo', () => {
    // a (50) e d (48) são os maiores; b e c empatam em 1 e vale o primeiro.
    expect(pickLayers(stack, 0, 150, 3)).toEqual({
      hovered: null,
      rows: ['d', 'b', 'a'],
      hidden: 1,
    })
  })
})
