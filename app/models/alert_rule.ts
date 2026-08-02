import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, computed } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'

export default class AlertRule extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare siteId: number | null

  @column()
  declare deviceId: number | null

  @column()
  declare monitorId: number | null

  @column()
  declare name: string

  @column()
  declare type: 'device_offline' | 'latency_high' | 'http_failure' | 'tcp_failure' | 'custom'

  @column({
    prepare: (value: Record<string, unknown>) => (value ? JSON.stringify(value) : null),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare condition: Record<string, unknown>

  @column()
  declare severity: 'info' | 'warning' | 'critical'

  @column()
  declare durationSeconds: number

  @computed()
  get isEnabled(): boolean {
    return this.enabled
  }

  @column()
  declare enabled: boolean

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Site)
  declare site: BelongsTo<typeof Site>

  @belongsTo(() => Device)
  declare device: BelongsTo<typeof Device>

  @belongsTo(() => Monitor)
  declare monitor: BelongsTo<typeof Monitor>
}
