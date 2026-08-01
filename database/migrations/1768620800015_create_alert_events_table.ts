import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'alert_events'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table.integer('alert_rule_id').unsigned().references('id').inTable('alert_rules').onDelete('CASCADE').notNullable()
      table.integer('device_id').unsigned().references('id').inTable('devices').onDelete('SET NULL').nullable()
      table.integer('monitor_id').unsigned().references('id').inTable('monitors').onDelete('SET NULL').nullable()
      table.string('status').notNullable()
      table.string('severity').notNullable()
      table.timestamp('started_at').notNullable()
      table.timestamp('resolved_at').nullable()
      table.text('message').nullable()
      table.jsonb('data').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
