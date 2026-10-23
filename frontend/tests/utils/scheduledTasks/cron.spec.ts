import { describe, expect, it } from 'vitest'
import {
  cronDraftErrors,
  defaultCronDraft,
  fromCron,
  toCron,
  type CronDraft,
} from '@/utils/scheduledTasks/cron'

function draft(change: Partial<CronDraft>): CronDraft {
  return { ...defaultCronDraft(), ...change }
}

describe('construtor de agenda (cron)', () => {
  it('cada modo amigável monta a expressão de 5 campos', () => {
    expect(toCron(draft({ mode: 'minutes', everyMinutes: 15 }))).toBe('*/15 * * * *')
    expect(toCron(draft({ mode: 'minutes', everyMinutes: 1 }))).toBe('* * * * *')
    expect(toCron(draft({ mode: 'hourly', everyHours: 1, minute: 30 }))).toBe('30 * * * *')
    expect(toCron(draft({ mode: 'hourly', everyHours: 6, minute: 0 }))).toBe('0 */6 * * *')
    expect(toCron(draft({ mode: 'daily', time: '03:05' }))).toBe('5 3 * * *')
    expect(toCron(draft({ mode: 'weekly', time: '08:00', weekdays: [5, 1, 2, 3, 4] }))).toBe(
      '0 8 * * 1-5'
    )
    expect(toCron(draft({ mode: 'weekly', time: '09:30', weekdays: [0, 6] }))).toBe('30 9 * * 6,0')
    expect(toCron(draft({ mode: 'monthly', time: '00:00', dayOfMonth: 15 }))).toBe('0 0 15 * *')
    expect(toCron(draft({ mode: 'custom', expression: '  0  3 * *   * ' }))).toBe('0 3 * * *')
  })

  it('reconhece a expressão de volta no modo certo — e o resto vai para o avançado', () => {
    for (const expression of [
      '*/15 * * * *',
      '30 * * * *',
      '0 */6 * * *',
      '5 3 * * *',
      '0 8 * * 1-5',
      '30 9 * * 6,0',
      '0 0 15 * *',
    ]) {
      const parsed = fromCron(expression)
      expect(parsed.mode, expression).not.toBe('custom')
      expect(toCron(parsed), expression).toBe(expression)
    }
    expect(fromCron('0 8 * * 1,3,5').weekdays).toEqual([1, 3, 5])
    expect(fromCron('0 7 * * 7').weekdays).toEqual([0])
    for (const advanced of ['0 0 1 1 *', '*/10 8-18 * * 1-5', '0 8,18 * * *', '@daily', 'x']) {
      const parsed = fromCron(advanced)
      expect(parsed.mode, advanced).toBe('custom')
      expect(toCron(parsed)).toBe(advanced)
    }
  })

  it('aponta o que falta antes de mandar ao servidor', () => {
    expect(cronDraftErrors(draft({ mode: 'weekly', weekdays: [] }))).toEqual([
      'Escolha pelo menos um dia da semana',
    ])
    expect(cronDraftErrors(draft({ mode: 'daily', time: '25:00' }))).toEqual([
      'Informe o horário no formato HH:MM',
    ])
    expect(cronDraftErrors(draft({ mode: 'custom', expression: ' ' }))).toEqual([
      'Digite a expressão cron',
    ])
    expect(cronDraftErrors(defaultCronDraft())).toEqual([])
  })
})
