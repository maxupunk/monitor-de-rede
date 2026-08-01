import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'devices'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table.integer('site_id').unsigned().references('id').inTable('sites').onDelete('CASCADE').notNullable()
      table.integer('network_id').unsigned().references('id').inTable('networks').onDelete('SET NULL').nullable()
      table.string('name').notNullable()
      table.string('type').notNullable()
      table.string('vendor').nullable()
      table.string('model').nullable()
      table.string('serial_number').nullable()
      table.text('description').nullable()
      table.string('status').defaultTo('unknown').notNullable()
      table.timestamp('last_seen_at').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
