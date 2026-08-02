import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'

export default class DeviceLink extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare sourceDeviceId: number

  @column()
  declare targetDeviceId: number

  @column()
  declare sourceInterfaceId: number | null

  @column()
  declare targetInterfaceId: number | null

  @column()
  declare linkType: string

  @column()
  declare discoveryMethod: string

  @column()
  declare confidence: number

  @column()
  declare confirmed: boolean

  @column.dateTime()
  declare lastSeenAt: DateTime | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Device, { foreignKey: 'sourceDeviceId' })
  declare sourceDevice: BelongsTo<typeof Device>

  @belongsTo(() => Device, { foreignKey: 'targetDeviceId' })
  declare targetDevice: BelongsTo<typeof Device>

  @belongsTo(() => DeviceInterface, { foreignKey: 'sourceInterfaceId' })
  declare sourceInterface: BelongsTo<typeof DeviceInterface>

  @belongsTo(() => DeviceInterface, { foreignKey: 'targetInterfaceId' })
  declare targetInterface: BelongsTo<typeof DeviceInterface>
}
