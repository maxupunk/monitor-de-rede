import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'metrics'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table.integer('device_id').unsigned().references('id').inTable('devices').onDelete('CASCADE').notNullable()
      table.integer('interface_id').unsigned().references('id').inTable('device_interfaces').onDelete('SET NULL').nullable()
      table.integer('monitor_id').unsigned().references('id').inTable('monitors').onDelete('SET NULL').nullable()
      table.string('name').notNullable()
      table.double('value').notNullable()
      table.string('unit').notNullable()
      table.timestamp('recorded_at').notNullable()

      table.timestamp('created_at').notNullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
