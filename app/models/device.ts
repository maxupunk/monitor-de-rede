import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import Site from '#models/site'
import Network from '#models/network'
import Monitor from '#models/monitor'

export default class Device extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare siteId: number

  @column()
  declare networkId: number | null

  @column()
  declare name: string

  @column()
  declare type: string

  @column()
  declare vendor: string | null

  @column()
  declare model: string | null

  @column()
  declare serialNumber: string | null

  @column()
  declare description: string | null

  @column()
  declare status: 'online' | 'offline' | 'warning' | 'unknown'

  @column.dateTime()
  declare lastSeenAt: DateTime | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Site)
  declare site: BelongsTo<typeof Site>

  @belongsTo(() => Network)
  declare network: BelongsTo<typeof Network>

  @hasMany(() => Monitor)
  declare monitors: HasMany<typeof Monitor>
}
