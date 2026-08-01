import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import DiscoveryRun from '#models/discovery_run'

export default class DiscoveryResult extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare discoveryRunId: number

  @column()
  declare ipAddress: string

  @column()
  declare macAddress: string | null

  @column()
  declare hostname: string | null

  @column()
  declare mdnsName: string | null

  @column()
  declare vendor: string | null

  @column()
  declare deviceType: string | null

  @column()
  declare confidence: number

  @column()
  declare status: 'pending' | 'accepted' | 'ignored' | 'merged'

  @column({
    prepare: (value: Record<string, unknown>) => (value ? JSON.stringify(value) : null),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare data: Record<string, unknown> | null

  @column.dateTime()
  declare firstSeenAt: DateTime

  @column.dateTime()
  declare lastSeenAt: DateTime

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => DiscoveryRun)
  declare discoveryRun: BelongsTo<typeof DiscoveryRun>
}
