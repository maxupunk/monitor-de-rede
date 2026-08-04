import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany, hasOne } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany, HasOne } from '@adonisjs/lucid/types/relations'
import Site from '#models/site'
import Network from '#models/network'
import Monitor from '#models/monitor'
import DeviceInterface from '#models/device_interface'
import Metric from '#models/metric'
import DeviceLink from '#models/device_link'
import VpnPeer from '#models/vpn_peer'

export default class Device extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare siteId: number | null

  @column()
  declare networkId: number | null

  @column()
  declare parentId: number | null

  @column()
  declare ipAddress: string | null

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
  declare isMonitored: boolean

  @column()
  declare snmpEnabled: boolean

  @column()
  declare snmpCommunity: string | null

  @column()
  declare snmpVersion: string | null

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

  @belongsTo(() => Device, { foreignKey: 'parentId' })
  declare parent: BelongsTo<typeof Device>

  @belongsTo(() => Network)
  declare network: BelongsTo<typeof Network>

  @hasMany(() => Monitor)
  declare monitors: HasMany<typeof Monitor>

  @hasMany(() => DeviceInterface)
  declare interfaces: HasMany<typeof DeviceInterface>

  @hasMany(() => Metric)
  declare metrics: HasMany<typeof Metric>

  @hasMany(() => DeviceLink, { foreignKey: 'sourceDeviceId' })
  declare outgoingLinks: HasMany<typeof DeviceLink>

  @hasMany(() => DeviceLink, { foreignKey: 'targetDeviceId' })
  declare incomingLinks: HasMany<typeof DeviceLink>

  @hasOne(() => VpnPeer)
  declare vpnPeer: HasOne<typeof VpnPeer>
}

