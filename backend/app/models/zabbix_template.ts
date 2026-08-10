import { DateTime } from 'luxon'
import { BaseModel, column, hasMany } from '@adonisjs/lucid/orm'
import type { HasMany } from '@adonisjs/lucid/types/relations'
import ZabbixTemplateItem from '#models/zabbix_template_item'

export default class ZabbixTemplate extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare zabbixUuid: string | null

  @column()
  declare name: string

  @column()
  declare description: string | null

  @column()
  declare zabbixVersion: string | null

  @column({
    prepare: (value: Record<string, unknown>) => JSON.stringify(value ?? {}),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare rawExport: Record<string, unknown>

  @column.dateTime()
  declare importedAt: DateTime

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @hasMany(() => ZabbixTemplateItem, { foreignKey: 'templateId' })
  declare items: HasMany<typeof ZabbixTemplateItem>
}
