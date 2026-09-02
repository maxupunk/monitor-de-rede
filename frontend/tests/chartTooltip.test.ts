import { describe, expect, it } from 'vitest'
import { chartTooltipStyle } from '../src/utils/chartTooltip.ts'

describe('chart tooltip positioning', () => {
  it('ancora à direita do cursor quando há espaço', () => {
    const style = chartTooltipStyle({
      x: 80,
      y: 100,
      containerWidth: 500,
      containerHeight: 240,
    })

    expect(style.left).toBe('96px')
    expect(style.right).toBeUndefined()
    expect(style.maxWidth).toBe('360px')
  })

  it('inverte para a esquerda e limita a largura na borda direita', () => {
    const style = chartTooltipStyle({
      x: 480,
      y: 100,
      containerWidth: 500,
      containerHeight: 240,
    })

    expect(style.right).toBe('36px')
    expect(style.left).toBeUndefined()
    expect(style.maxWidth).toBe('360px')
    expect(style.whiteSpace).toBe('normal')
  })

  it('usa somente o espaço restante em gráficos estreitos', () => {
    const style = chartTooltipStyle({
      x: 150,
      y: 20,
      containerWidth: 240,
      containerHeight: 180,
      maxWidth: 360,
    })

    expect(style.right).toBe('106px')
    expect(style.maxWidth).toBe('126px')
    expect(style.top).toBe('36px')
  })

  it('inverte para cima quando o cursor se aproxima da borda inferior', () => {
    const style = chartTooltipStyle({
      x: 100,
      y: 220,
      containerWidth: 500,
      containerHeight: 240,
    })

    expect(style.bottom).toBe('36px')
    expect(style.top).toBeUndefined()
    expect(style.maxHeight).toBe('196px')
  })
})
