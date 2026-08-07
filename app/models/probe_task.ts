import { DateTime } from 'luxon'
import { BaseModel, column } from '@adonisjs/lucid/orm'

/**
 * Tarefa pendente de um probe. É a ponte entre o scheduler, que enfileira, e o
 * processo HTTP, que entrega ao agente — ver a migration `create_probe_tasks_table`.
 */
export default class ProbeTaskRecord extends BaseModel {
  static table = 'probe_tasks'

  // O driver do Postgres devolve bigint como string.
  @column({ isPrimary: true, consume: (value: string | number) => Number(value) })
  declare id: number

  @column()
  declare probeId: number

  @column()
  declare monitorId: number

  @column()
  declare taskId: string

  @column()
  declare type: 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'

  @column()
  declare timeoutMs: number

  @column({
    prepare: (value: Record<string, unknown>) => JSON.stringify(value ?? {}),
    consume: (value: string | Record<string, unknown>) =>
      typeof value === 'string' ? JSON.parse(value) : value || {},
  })
  declare payload: Record<string, unknown>

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime
}
