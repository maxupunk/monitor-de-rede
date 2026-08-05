import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

/**
 * Registro da caixa de saída de eventos consumida pelo EventRelay.
 * Ver database/migrations/..._create_event_outbox_table.ts.
 */
export default class EventOutbox extends BaseModel {
  static table = 'event_outbox'

  // O driver do Postgres devolve bigint como string; o relay compara cursores
  // numericamente, então a conversão precisa ser explícita.
  @column({ isPrimary: true, consume: (value: string | number) => Number(value) })
  declare id: number

  @column()
  declare type: string

  @column()
  declare origin: string

  @column({
    prepare: (value: Record<string, unknown>) => JSON.stringify(value ?? {}),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare payload: Record<string, unknown>

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime
}
