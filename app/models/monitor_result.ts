import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

export default class MonitorResult extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare monitorId: number

  @column()
  declare probeId: number | null

  @column()
  declare status: 'up' | 'down' | 'warning' | 'unknown'

  @column.dateTime()
  declare startedAt: DateTime

  @column.dateTime()
  declare finishedAt: DateTime

  @column()
  declare durationMs: number

  @column()
  declare latencyMs: number | null

  @column()
  declare message: string | null

  @column({
    prepare: (value: Record<string, unknown>) => JSON.stringify(value),
    consume: (value: string) => (value ? JSON.parse(value) : {}),
  })
  declare data: Record<string, unknown>

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime
}
