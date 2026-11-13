import { describe, expect, it } from 'vitest'
import { toggleRating } from '@/utils/aiRating'

describe('avaliação de uma resposta', () => {
  it('o mesmo polegar desfaz e o outro troca', () => {
    expect(toggleRating(null, 'up')).toEqual({ value: 'up' })
    expect(toggleRating({ value: 'up' }, 'up')).toBeNull()
    expect(toggleRating({ value: 'up' }, 'down', 'Incompleta')).toEqual({
      value: 'down',
      reason: 'Incompleta',
    })
  })

  it('no 👎, trocar o motivo atualiza; sem motivo, desfaz', () => {
    const ruim = { value: 'down' as const, reason: 'Resposta errada' }
    expect(toggleRating(ruim, 'down', 'Incompleta')).toEqual({
      value: 'down',
      reason: 'Incompleta',
    })
    expect(toggleRating(ruim, 'down', 'Resposta errada')).toBeNull()
    expect(toggleRating(ruim, 'down')).toBeNull()
    expect(toggleRating(null, 'down', '   ')).toEqual({ value: 'down', reason: null })
  })
})
