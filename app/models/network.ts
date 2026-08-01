import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import Site from '#models/site'
import Probe from '#models/probe'
import Device from '#models/device'

export default class Network extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare siteId: number

  @column()
  declare probeId: number | null

  @column()
  declare name: string

  @column()
  declare cidr: string

  @column()
  declare gateway: string | null

  @column()
  declare vlan: number | null

  @column({
    prepare: (value: unknown) => (value ? JSON.stringify(value) : null),
    consume: (value: string | unknown) => (typeof value === 'string' ? JSON.parse(value) : value),
  })
  declare dnsServers: string[] | null

  @column()
  declare scanEnabled: boolean

  @column()
  declare scanInterval: number

  @column()
  declare active: boolean

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Site)
  declare site: BelongsTo<typeof Site>

  @belongsTo(() => Probe)
  declare probe: BelongsTo<typeof Probe>

  @hasMany(() => Device)
  declare devices: HasMany<typeof Device>
}
