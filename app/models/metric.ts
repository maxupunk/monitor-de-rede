import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import Monitor from '#models/monitor'

export default class Metric extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare deviceId: number

  @column()
  declare interfaceId: number | null

  @column()
  declare monitorId: number | null

  @column()
  declare name: string

  @column()
  declare value: number

  @column()
  declare unit: string

  @column.dateTime()
  declare recordedAt: DateTime

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @belongsTo(() => Device)
  declare device: BelongsTo<typeof Device>

  @belongsTo(() => DeviceInterface, { foreignKey: 'interfaceId' })
  declare interface: BelongsTo<typeof DeviceInterface>

  @belongsTo(() => Monitor)
  declare monitor: BelongsTo<typeof Monitor>
}
