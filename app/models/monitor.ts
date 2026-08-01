import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import Device from '#models/device'
import Probe from '#models/probe'
import MonitorResult from '#models/monitor_result'

export default class Monitor extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare deviceId: number

  @column()
  declare probeId: number | null

  @column()
  declare type: 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'

  @column()
  declare name: string

  @column({
    prepare: (value: Record<string, unknown>) => (value ? JSON.stringify(value) : null),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare configuration: Record<string, unknown>

  @column()
  declare intervalSeconds: number

  @column()
  declare timeoutSeconds: number

  @column()
  declare retryCount: number

  @column()
  declare enabled: boolean

  @column.dateTime()
  declare nextRunAt: DateTime | null

  @column.dateTime()
  declare lastRunAt: DateTime | null

  @column()
  declare status: 'up' | 'down' | 'warning' | 'unknown'

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Device)
  declare device: BelongsTo<typeof Device>

  @belongsTo(() => Probe)
  declare probe: BelongsTo<typeof Probe>

  @hasMany(() => MonitorResult)
  declare results: HasMany<typeof MonitorResult>
}
