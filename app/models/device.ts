import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

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
}
