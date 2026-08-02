import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import AlertRule from '#models/alert_rule'
import Device from '#models/device'
import Monitor from '#models/monitor'

export default class AlertEvent extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare alertRuleId: number

  @column()
  declare deviceId: number | null

  @column()
  declare monitorId: number | null

  @column()
  declare status: 'active' | 'acknowledged' | 'silenced' | 'resolved'

  @column()
  declare severity: 'info' | 'warning' | 'critical'

  @column.dateTime()
  declare startedAt: DateTime

  @column.dateTime()
  declare resolvedAt: DateTime | null

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

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => AlertRule)
  declare alertRule: BelongsTo<typeof AlertRule>

  @belongsTo(() => Device)
  declare device: BelongsTo<typeof Device>

  @belongsTo(() => Monitor)
  declare monitor: BelongsTo<typeof Monitor>
}
