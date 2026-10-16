import { describe, expect, it } from 'vitest'
import {
  DEFAULT_SPEC,
  estimateSeconds,
  estimatedDurationText,
  intervalLabel,
  legExpectedSeconds,
  normalizeSpec,
  specSummary,
  validateSchedule,
  validateSpec,
  windowLabel,
} from '@/utils/linkTests/spec'
import type { LinkTestScheduleInput } from '@/types/linkTests'

function schedule(patch: Partial<LinkTestScheduleInput> = {}): LinkTestScheduleInput {
  return {
    name: 'Torre',
    kind: 'link',
    linkId: 1,
    sourceDeviceId: 1,
    targetDeviceId: 2,
    intervalMinutes: 60,
    windowStart: null,
    windowEnd: null,
    enabled: true,
    ...patch,
  }
}

describe('parâmetros do teste', () => {
  it('padrões do ADR: 5 s, ambos os sentidos, 1 fluxo, sem limite', () => {
    expect(normalizeSpec()).toEqual(DEFAULT_SPEC)
    expect(DEFAULT_SPEC).toEqual({ durationS: 5, direction: 'both', streams: 1, limitMbps: null })
  })

  it('limita a 1–60 s e 1–8 fluxos, descarta limite inválido', () => {
    expect(normalizeSpec({ durationS: 0, streams: 20, limitMbps: -3 })).toEqual({
      durationS: 1,
      direction: 'both',
      streams: 8,
      limitMbps: null,
    })
    expect(
      normalizeSpec({ durationS: 90.4, direction: 'reverse', limitMbps: 100.4 })
    ).toMatchObject({ durationS: 60, direction: 'reverse', limitMbps: 100 })
  })

  it('valida com mensagens em português', () => {
    expect(validateSpec({ durationS: 5, streams: 1, limitMbps: null })).toEqual([])
    expect(validateSpec({ durationS: 61 })).toEqual([
      'A duração vai de 1 a 60 segundos por sentido.',
    ])
    expect(validateSpec({ streams: 9 })[0]).toContain('Fluxos paralelos')
    expect(validateSpec({ limitMbps: 0 })[0]).toContain('maior que zero')
  })

  it('estima a duração: pernas × (duração × sentidos + preparo)', () => {
    expect(estimateSeconds({ durationS: 5 })).toBe(16)
    expect(estimateSeconds({ durationS: 10, direction: 'forward' }, 3)).toBe(48)
    expect(legExpectedSeconds({ durationS: 5, runnable: false }, 'both')).toBe(0)
    expect(estimatedDurationText(16)).toBe('cerca de 16 s')
    expect(estimatedDurationText(70)).toBe('cerca de 1 min 10 s')
  })

  it('resumo legível', () => {
    expect(specSummary({})).toBe('5 s por sentido · ida e volta')
    expect(specSummary({ durationS: 10, direction: 'forward', streams: 4, limitMbps: 50 })).toBe(
      '10 s por sentido · só ida · 4 fluxos · até 50 Mbps'
    )
  })
})

describe('agendamento', () => {
  it('rótulos de intervalo e janela', () => {
    expect(intervalLabel(15)).toBe('a cada 15 min')
    expect(intervalLabel(360)).toBe('a cada 6 h')
    expect(intervalLabel(1440)).toBe('a cada 1 dia')
    expect(intervalLabel(90)).toBe('a cada 90 min')
    expect(windowLabel('22:00', '06:00')).toBe('das 22:00 às 06:00')
    expect(windowLabel(null, '06:00')).toBeNull()
  })

  it('valida nome, intervalo mínimo de 15 min e janela completa', () => {
    expect(validateSchedule(schedule())).toEqual([])
    expect(validateSchedule(schedule({ name: '  ' }))).toContain('Dê um nome ao agendamento.')
    expect(validateSchedule(schedule({ intervalMinutes: 10 }))).toContain(
      'O intervalo mínimo é de 15 minutos.'
    )
    expect(validateSchedule(schedule({ windowStart: '22:00' }))[0]).toContain('início e o fim')
    expect(validateSchedule(schedule({ windowStart: '25:00', windowEnd: '06:00' }))[0]).toContain(
      'HH:MM'
    )
    expect(validateSchedule(schedule({ windowStart: '06:00', windowEnd: '06:00' }))[0]).toContain(
      'iguais'
    )
    expect(validateSchedule(schedule({ spec: { durationS: 0 } }))[0]).toContain('duração')
  })
})
