import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

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

  @column()
  declare dnsServers: string | null

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
}
