import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import ZabbixTemplate from '#models/zabbix_template'

export type ZabbixItemValueType = 'FLOAT' | 'UNSIGNED' | 'TEXT' | 'CHAR' | 'LOG'

export default class ZabbixTemplateItem extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare templateId: number

  @column()
  declare zabbixUuid: string | null

  @column()
  declare name: string

  /** Item key_ do Zabbix (ex: "V.bat") — usado como Metric.name ao coletar via SNMP. */
  @column()
  declare key: string

  @column()
  declare snmpOid: string

  @column()
  declare valueType: ZabbixItemValueType

  @column()
  declare units: string | null

  /** Fator de multiplicação do preprocessing MULTIPLIER do Zabbix, se houver. */
  @column()
  declare multiplier: number | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @belongsTo(() => ZabbixTemplate, { foreignKey: 'templateId' })
  declare template: BelongsTo<typeof ZabbixTemplate>
}
