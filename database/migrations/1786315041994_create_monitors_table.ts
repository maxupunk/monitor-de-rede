import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'monitors'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      /**
       * Nem toda checagem pertence a um equipamento: medir a latência de um
       * servidor DNS público ou a disponibilidade de um site externo não
       * depende de um dispositivo cadastrado.
       */
      table
        .integer('device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('CASCADE')
        .nullable()
      table
        .integer('probe_id')
        .unsigned()
        .references('id')
        .inTable('probes')
        .onDelete('SET NULL')
        .nullable()
      table.string('type').notNullable()
      table.string('name').notNullable()
      table.jsonb('configuration').notNullable()
      table.integer('interval_seconds').defaultTo(15).notNullable()
      table.integer('timeout_seconds').defaultTo(10).notNullable()
      table.integer('retry_count').defaultTo(3).notNullable()
      table.boolean('enabled').defaultTo(true).notNullable()
      table.timestamp('next_run_at').nullable()
      table.timestamp('last_run_at').nullable()
      table.string('status').defaultTo('unknown').notNullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      /** Seleção dos monitores vencidos: o laço central do `scheduler:run`. */
      table.index(['enabled', 'next_run_at'], 'monitors_enabled_next_run_at_index')
      table.index(['device_id', 'enabled'], 'monitors_device_id_enabled_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
