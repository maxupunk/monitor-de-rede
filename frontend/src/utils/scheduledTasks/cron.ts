/**
 * Construtor de agenda das tarefas: modos amigáveis ("todo dia às 03:00",
 * "dias da semana") ↔ expressão cron de 5 campos, na hora local do servidor.
 *
 * Só monta e reconhece a forma. A frase em português e as próximas
 * execuções vêm do backend (`POST /scheduled-tasks/preview`), que é a fonte
 * única delas.
 */

export type CronMode = 'minutes' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'custom'

export interface CronDraft {
  mode: CronMode
  /** `minutes`: a cada N minutos (1–59). */
  everyMinutes: number
  /** `hourly`: a cada N horas (1–23)… */
  everyHours: number
  /** …no minuto M (0–59). */
  minute: number
  /** `daily`, `weekly`, `monthly`: "HH:MM". */
  time: string
  /** `weekly`: 0 = domingo … 6 = sábado. */
  weekdays: number[]
  /** `monthly`: dia do mês (1–31). */
  dayOfMonth: number
  /** `custom`: a expressão como digitada. */
  expression: string
}

export const CRON_MODES: ReadonlyArray<{ value: CronMode; title: string; icon: string }> = [
  { value: 'minutes', title: 'A cada X minutos', icon: 'mdi-timer-outline' },
  { value: 'hourly', title: 'De hora em hora', icon: 'mdi-clock-outline' },
  { value: 'daily', title: 'Todo dia', icon: 'mdi-calendar-today' },
  { value: 'weekly', title: 'Dias da semana', icon: 'mdi-calendar-week' },
  { value: 'monthly', title: 'Todo mês', icon: 'mdi-calendar-month' },
  { value: 'custom', title: 'Avançado (cron)', icon: 'mdi-code-braces' },
]

export const MINUTE_PRESETS: readonly number[] = [5, 10, 15, 30]

/** Segunda primeiro, como no calendário de trabalho. */
export const WEEKDAYS: ReadonlyArray<{ value: number; short: string; long: string }> = [
  { value: 1, short: 'Seg', long: 'Segunda' },
  { value: 2, short: 'Ter', long: 'Terça' },
  { value: 3, short: 'Qua', long: 'Quarta' },
  { value: 4, short: 'Qui', long: 'Quinta' },
  { value: 5, short: 'Sex', long: 'Sexta' },
  { value: 6, short: 'Sáb', long: 'Sábado' },
  { value: 0, short: 'Dom', long: 'Domingo' },
]

export const WORKDAYS: readonly number[] = [1, 2, 3, 4, 5]
export const WEEKEND: readonly number[] = [6, 0]

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/

export function defaultCronDraft(): CronDraft {
  return {
    mode: 'daily',
    everyMinutes: 15,
    everyHours: 1,
    minute: 0,
    time: '03:00',
    weekdays: [...WORKDAYS],
    dayOfMonth: 1,
    expression: '0 3 * * *',
  }
}

function pad(value: number): string {
  return String(value).padStart(2, '0')
}

function hourMinute(time: string): [number, number] {
  const match = TIME.exec(time)
  return match ? [Number(match[1]), Number(match[2])] : [0, 0]
}

/** Segunda primeiro; domingo (0) no fim. */
function sortWeekdays(days: readonly number[]): number[] {
  return [...new Set(days)].sort((a, b) => ((a + 6) % 7) - ((b + 6) % 7))
}

function weekdayField(days: readonly number[]): string {
  const sorted = sortWeekdays(days)
  if (sorted.join(',') === WORKDAYS.join(',')) return '1-5'
  return sorted.join(',')
}

/** A expressão de 5 campos do rascunho. */
export function toCron(draft: CronDraft): string {
  const [hour, minute] = hourMinute(draft.time)
  switch (draft.mode) {
    case 'minutes':
      return draft.everyMinutes <= 1 ? '* * * * *' : `*/${draft.everyMinutes} * * * *`
    case 'hourly':
      return draft.everyHours <= 1
        ? `${draft.minute} * * * *`
        : `${draft.minute} */${draft.everyHours} * * *`
    case 'daily':
      return `${minute} ${hour} * * *`
    case 'weekly':
      return `${minute} ${hour} * * ${weekdayField(draft.weekdays)}`
    case 'monthly':
      return `${minute} ${hour} ${draft.dayOfMonth} * *`
    case 'custom':
      return draft.expression.trim().replace(/\s+/g, ' ')
  }
}

function parseWeekdays(field: string): number[] | null {
  const days: number[] = []
  for (const part of field.split(',')) {
    const range = /^(\d)-(\d)$/.exec(part)
    if (range) {
      const start = Number(range[1])
      const end = Number(range[2])
      if (start > end) return null
      for (let day = start; day <= end; day += 1) days.push(day % 7)
    } else if (/^\d$/.test(part)) {
      days.push(Number(part) % 7)
    } else {
      return null
    }
  }
  return days.some((day) => day > 6) ? null : sortWeekdays(days)
}

function inRange(value: number, min: number, max: number): boolean {
  return Number.isInteger(value) && value >= min && value <= max
}

/**
 * O rascunho de uma expressão. O que não cabe num modo amigável abre no
 * "Avançado", sem perder nada.
 */
export function fromCron(expression: string): CronDraft {
  const draft = defaultCronDraft()
  const normalized = expression.trim().replace(/\s+/g, ' ')
  const custom: CronDraft = { ...draft, mode: 'custom', expression: normalized }
  const fields = normalized.split(' ')
  if (fields.length !== 5) return custom
  const [minute, hour, dayOfMonth, month, dayOfWeek] = fields as [
    string,
    string,
    string,
    string,
    string,
  ]
  if (month !== '*') return custom
  const minuteNumber = /^\d+$/.test(minute) ? Number(minute) : NaN
  const hourNumber = /^\d+$/.test(hour) ? Number(hour) : NaN
  const time =
    inRange(minuteNumber, 0, 59) && inRange(hourNumber, 0, 23)
      ? `${pad(hourNumber)}:${pad(minuteNumber)}`
      : null

  if (hour === '*' && dayOfMonth === '*' && dayOfWeek === '*') {
    if (minute === '*')
      return { ...draft, mode: 'minutes', everyMinutes: 1, expression: normalized }
    const step = /^\*\/(\d+)$/.exec(minute)
    if (step && inRange(Number(step[1]), 1, 59)) {
      return { ...draft, mode: 'minutes', everyMinutes: Number(step[1]), expression: normalized }
    }
    if (inRange(minuteNumber, 0, 59)) {
      return {
        ...draft,
        mode: 'hourly',
        everyHours: 1,
        minute: minuteNumber,
        expression: normalized,
      }
    }
    return custom
  }
  const hourStep = /^\*\/(\d+)$/.exec(hour)
  if (hourStep && dayOfMonth === '*' && dayOfWeek === '*' && inRange(minuteNumber, 0, 59)) {
    const everyHours = Number(hourStep[1])
    if (!inRange(everyHours, 1, 23)) return custom
    return { ...draft, mode: 'hourly', everyHours, minute: minuteNumber, expression: normalized }
  }
  if (!time) return custom
  if (dayOfMonth === '*' && dayOfWeek === '*') {
    return { ...draft, mode: 'daily', time, expression: normalized }
  }
  if (dayOfMonth === '*') {
    const weekdays = parseWeekdays(dayOfWeek)
    if (!weekdays || weekdays.length === 0) return custom
    return { ...draft, mode: 'weekly', time, weekdays, expression: normalized }
  }
  if (dayOfWeek === '*' && /^\d+$/.test(dayOfMonth) && inRange(Number(dayOfMonth), 1, 31)) {
    return {
      ...draft,
      mode: 'monthly',
      time,
      dayOfMonth: Number(dayOfMonth),
      expression: normalized,
    }
  }
  return custom
}

/** O que impede montar a expressão; vazio quando o rascunho está certo. */
export function cronDraftErrors(draft: CronDraft): string[] {
  const errors: string[] = []
  if (['daily', 'weekly', 'monthly'].includes(draft.mode) && !TIME.test(draft.time)) {
    errors.push('Informe o horário no formato HH:MM')
  }
  if (draft.mode === 'minutes' && !inRange(draft.everyMinutes, 1, 59)) {
    errors.push('O intervalo em minutos vai de 1 a 59')
  }
  if (draft.mode === 'hourly') {
    if (!inRange(draft.everyHours, 1, 23)) errors.push('O intervalo em horas vai de 1 a 23')
    if (!inRange(draft.minute, 0, 59)) errors.push('O minuto vai de 0 a 59')
  }
  if (draft.mode === 'weekly' && draft.weekdays.length === 0) {
    errors.push('Escolha pelo menos um dia da semana')
  }
  if (draft.mode === 'monthly' && !inRange(draft.dayOfMonth, 1, 31)) {
    errors.push('O dia do mês vai de 1 a 31')
  }
  if (draft.mode === 'custom' && draft.expression.trim() === '') {
    errors.push('Digite a expressão cron')
  }
  return errors
}
