import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'device_interfaces'

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
      table.integer('snmp_index').nullable()
      table.string('name').notNullable()
      table.string('description').nullable()
      table.string('alias').nullable()
      table.string('mac_address').nullable()
      table.string('type').nullable()
      table.bigInteger('speed').nullable()
      table.string('admin_status').nullable()
      table.string('oper_status').nullable()
      table.timestamp('last_seen_at').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
