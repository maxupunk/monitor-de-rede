import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

export default class Monitor extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare deviceId: number

  @column()
  declare probeId: number | null

  @column()
  declare type: 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'

  @column()
  declare name: string

  @column({
    prepare: (value: Record<string, unknown>) => JSON.stringify(value),
    consume: (value: string) => (value ? JSON.parse(value) : {}),
  })
  declare configuration: Record<string, unknown>

  @column()
  declare intervalSeconds: number

  @column()
  declare timeoutSeconds: number

  @column()
  declare retryCount: number

  @column()
  declare enabled: boolean

  @column.dateTime()
  declare nextRunAt: DateTime | null

  @column.dateTime()
  declare lastRunAt: DateTime | null

  @column()
  declare status: 'up' | 'down' | 'warning' | 'unknown'

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime
}
