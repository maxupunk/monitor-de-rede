import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'networks'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('site_id')
        .unsigned()
        .references('id')
        .inTable('sites')
        .onDelete('CASCADE')
        .notNullable()
      table
        .integer('probe_id')
        .unsigned()
        .references('id')
        .inTable('probes')
        .onDelete('SET NULL')
        .nullable()
      table.string('name').notNullable()
      table.string('cidr').notNullable()
      table.string('gateway').nullable()
      table.integer('vlan').nullable()
      table.jsonb('dns_servers').nullable()
      table.boolean('scan_enabled').defaultTo(true).notNullable()
      table.integer('scan_interval').defaultTo(3600).notNullable()
      table.boolean('active').defaultTo(true).notNullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
