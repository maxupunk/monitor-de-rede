import { DateTime } from 'luxon'
import { BaseModel, column, hasMany } from '@adonisjs/lucid/orm'
import type { HasMany } from '@adonisjs/lucid/types/relations'
import Network from '#models/network'
import Probe from '#models/probe'
import Device from '#models/device'

export default class Site extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare name: string

  @column()
  declare description: string | null

  @column()
  declare location: string | null

  @column()
  declare active: boolean

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @hasMany(() => Network)
  declare networks: HasMany<typeof Network>

  @hasMany(() => Probe)
  declare probes: HasMany<typeof Probe>

  @hasMany(() => Device)
  declare devices: HasMany<typeof Device>
}
