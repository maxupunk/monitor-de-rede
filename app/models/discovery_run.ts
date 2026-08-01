import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import Network from '#models/network'
import Probe from '#models/probe'
import DiscoveryResult from '#models/discovery_result'

export default class DiscoveryRun extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare networkId: number

  @column()
  declare probeId: number | null

  @column()
  declare status: 'pending' | 'running' | 'completed' | 'failed'

  @column.dateTime()
  declare startedAt: DateTime

  @column.dateTime()
  declare finishedAt: DateTime | null

  @column({
    prepare: (value: Record<string, unknown>) => (value ? JSON.stringify(value) : null),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare configuration: Record<string, unknown> | null

  @column()
  declare error: string | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Network)
  declare network: BelongsTo<typeof Network>

  @belongsTo(() => Probe)
  declare probe: BelongsTo<typeof Probe>

  @hasMany(() => DiscoveryResult)
  declare results: HasMany<typeof DiscoveryResult>
}
