import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'alert_events'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('alert_rule_id')
        .unsigned()
        .references('id')
        .inTable('alert_rules')
        .onDelete('CASCADE')
        .nullable()
      table
        .integer('device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('SET NULL')
        .nullable()
      table
        .integer('monitor_id')
        .unsigned()
        .references('id')
        .inTable('monitors')
        .onDelete('SET NULL')
        .nullable()
      /**
       * `scope_key` identifica o alvo concreto do alerta (`monitor:12`,
       * `interface:34`, ...). Sem ele não é possível deduplicar nem normalizar
       * alertas de alvos que não são monitores — duas interfaces do mesmo
       * dispositivo colapsariam no mesmo evento.
       */
      table.string('scope_key').nullable()
      table.string('status').notNullable()
      table.string('severity').notNullable()
      table.timestamp('started_at').notNullable()
      table.timestamp('resolved_at').nullable()
      table.text('message').nullable()
      table.jsonb('data').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      table.index(['scope_key'], 'alert_events_scope_key_index')

      /**
       * Deduplicação: procura o evento aberto da regra para aquele alvo a cada
       * resultado de monitor processado.
       */
      table.index(['alert_rule_id', 'scope_key', 'status'], 'alert_events_rule_scope_status_index')
      table.index(['device_id', 'created_at'], 'alert_events_device_created_index')
      table.index(['monitor_id', 'created_at'], 'alert_events_monitor_created_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
