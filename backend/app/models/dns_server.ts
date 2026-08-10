import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

/**
 * Servidor DNS cadastrado pelo usuário. Alimenta o autocomplete do formulário
 * de monitores e a comparação de latência exibida no dashboard.
 */
export default class DnsServer extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare name: string

  /** IP, `ip:porta` (UDP/TCP) ou endpoint https (DoH) */
  @column()
  declare address: string

  @column()
  declare protocol: 'udp' | 'tcp' | 'doh'

  @column()
  declare isDefault: boolean

  @column()
  declare description: string | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime
}
