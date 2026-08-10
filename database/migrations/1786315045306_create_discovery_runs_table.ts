import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'discovery_runs'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('network_id')
        .unsigned()
        .references('id')
        .inTable('networks')
        .onDelete('CASCADE')
        .notNullable()
      table
        .integer('probe_id')
        .unsigned()
        .references('id')
        .inTable('probes')
        .onDelete('SET NULL')
        .nullable()
      table.string('status').notNullable()
      table.timestamp('started_at').notNullable()
      table.timestamp('finished_at').nullable()
      table.jsonb('configuration').nullable()
      table.text('error').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      /** Próxima varredura pendente — consultado a cada ciclo do scheduler. */
      table.index(['status', 'id'], 'discovery_runs_status_id_index')
      table.index(['network_id', 'status'], 'discovery_runs_network_status_index')
      table.index(['created_at'], 'discovery_runs_created_at_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
