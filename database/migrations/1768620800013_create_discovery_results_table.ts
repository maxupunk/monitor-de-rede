import { BaseSchema } from '@adonisjs/lucid/schema'

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
      table.string('status').defaultTo('pending').notNullable()
      table.jsonb('data').nullable()
      table.timestamp('first_seen_at').notNullable()
      table.timestamp('last_seen_at').notNullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
