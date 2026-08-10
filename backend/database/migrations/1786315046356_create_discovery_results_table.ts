import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * Cache do último scan de cada rede, sem histórico persistente de status: um
 * resultado existe enquanto não foi transformado em device, e a verificação de
 * "já adicionado" é feita comparando o IP com a tabela `devices`.
 */
export default class extends BaseSchema {
  protected tableName = 'discovery_results'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('discovery_run_id')
        .unsigned()
        .references('id')
        .inTable('discovery_runs')
        .onDelete('CASCADE')
        .notNullable()
      table.string('ip_address').notNullable()
      table.string('mac_address').nullable()
      table.string('hostname').nullable()
      table.string('mdns_name').nullable()
      table.string('vendor').nullable()
      table.string('device_type').nullable()
      table.integer('confidence').defaultTo(0).notNullable()
      table.jsonb('data').nullable()
      table.timestamp('first_seen_at').notNullable()
      table.timestamp('last_seen_at').notNullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      /** Sustenta o `withCount('results')` da tela de Descoberta e o CASCADE. */
      table.index(['discovery_run_id'], 'discovery_results_discovery_run_id_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
