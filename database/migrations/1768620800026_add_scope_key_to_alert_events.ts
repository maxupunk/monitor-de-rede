import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * `scope_key` identifica o alvo concreto do alerta (`monitor:12`,
 * `interface:34`, ...). Sem ele não era possível deduplicar nem normalizar
 * alertas de alvos que não são monitores — duas interfaces do mesmo
 * dispositivo colapsariam no mesmo evento.
 */
export default class extends BaseSchema {
  protected tableName = 'alert_events'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.string('scope_key').nullable()
      table.index(['scope_key'], 'alert_events_scope_key_index')
    })

    // Eventos anteriores à coluna: todos nasceram de monitores.
    this.defer(async (db) => {
      await db
        .from(this.tableName)
        .whereNotNull('monitor_id')
        .update({ scope_key: db.raw("'monitor:' || monitor_id") })
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropIndex(['scope_key'], 'alert_events_scope_key_index')
      table.dropColumn('scope_key')
    })
  }
}
