import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import Monitor from '#models/monitor'
import Probe from '#models/probe'

export default class MonitorResult extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare monitorId: number

  @column()
  declare probeId: number | null

  @column()
  declare status: 'up' | 'down' | 'warning' | 'unknown' | 'disabled'

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
    prepare: (value: Record<string, unknown>) => (value ? JSON.stringify(value) : null),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare data: Record<string, unknown> | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @belongsTo(() => Monitor)
  declare monitor: BelongsTo<typeof Monitor>

  @belongsTo(() => Probe)
  declare probe: BelongsTo<typeof Probe>
}
