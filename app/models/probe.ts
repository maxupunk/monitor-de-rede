import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

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
  declare status: 'online' | 'offline' | 'busy' | 'revoked'

  @column()
  declare version: string

  @column.dateTime()
  declare lastSeenAt: DateTime | null

  @column.dateTime({ autoCreate: true })
  declare registeredAt: DateTime

  @column.dateTime()
  declare revokedAt: DateTime | null

  @column({
    prepare: (value: Record<string, unknown>) => JSON.stringify(value),
    consume: (value: string) => (value ? JSON.parse(value) : {}),
  })
  declare configuration: Record<string, unknown>
}
