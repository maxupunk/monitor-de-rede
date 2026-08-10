import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import Device from '#models/device'
import Metric from '#models/metric'

export default class DeviceInterface extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare deviceId: number

  @column()
  declare snmpIndex: number | null

  @column()
  declare name: string

  @column()
  declare description: string | null

  @column()
  declare alias: string | null

  @column()
  declare macAddress: string | null

  @column()
  declare type: string | null

  @column()
  declare speed: number | null

  @column()
  declare adminStatus: string | null

  @column()
  declare operStatus: string | null

  @column.dateTime()
  declare lastSeenAt: DateTime | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Device)
  declare device: BelongsTo<typeof Device>

  @hasMany(() => Metric, { foreignKey: 'interfaceId' })
  declare metrics: HasMany<typeof Metric>
}
