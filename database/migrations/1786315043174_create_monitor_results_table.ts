import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'monitor_results'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('monitor_id')
        .unsigned()
        .references('id')
        .inTable('monitors')
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
      table.timestamp('finished_at').notNullable()
      table.integer('duration_ms').notNullable()
      table.float('latency_ms').nullable()
      table.text('message').nullable()
      table.jsonb('data').nullable()

      table.timestamp('created_at').notNullable()

      /**
       * Tabela de maior volume e de escrita quente — só dois índices, ambos
       * pagos por consultas de laço: o histórico por monitor (listagem,
       * sparkline, dashboard DNS) e a purga do `DataPrunerService`.
       * O B-tree ascendente atende o `ORDER BY started_at DESC` lendo ao contrário.
       */
      table.index(['monitor_id', 'started_at'], 'monitor_results_monitor_started_index')
      table.index(['created_at'], 'monitor_results_created_at_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
