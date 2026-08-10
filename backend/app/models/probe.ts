import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import Site from '#models/site'
import Network from '#models/network'
import Monitor from '#models/monitor'

export default class Probe extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare siteId: number | null

  @column()
  declare name: string

  @column()
  declare tokenHash: string

  @column()
  declare status: 'online' | 'offline' | 'busy' | 'revoked' | 'pending'

  @column()
  declare version: string | null

  @column.dateTime()
  declare lastSeenAt: DateTime | null

  @column.dateTime()
  declare registeredAt: DateTime | null

  @column.dateTime()
  declare revokedAt: DateTime | null

  @column({
    prepare: (value: Record<string, unknown>) => (value ? JSON.stringify(value) : null),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare configuration: Record<string, unknown> | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Site)
  declare site: BelongsTo<typeof Site>

  @hasMany(() => Network)
  declare networks: HasMany<typeof Network>

  @hasMany(() => Monitor)
  declare monitors: HasMany<typeof Monitor>
}
