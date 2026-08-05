import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'alert_rules'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('site_id')
        .unsigned()
        .references('id')
        .inTable('sites')
        .onDelete('CASCADE')
        .nullable()
      table
        .integer('device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('CASCADE')
        .nullable()
      table
        .integer('monitor_id')
        .unsigned()
        .references('id')
        .inTable('monitors')
        .onDelete('CASCADE')
        .nullable()
      table.string('name').notNullable()
      table.string('type').notNullable()
      table.jsonb('condition').notNullable()
      table.string('severity').notNullable()
      table.integer('duration_seconds').defaultTo(0).notNullable()
      table.boolean('enabled').defaultTo(true).notNullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
