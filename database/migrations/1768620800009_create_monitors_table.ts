import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'monitors'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('CASCADE')
        .notNullable()
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
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
