import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'zabbix_template_items'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('template_id')
        .unsigned()
        .references('id')
        .inTable('zabbix_templates')
        .onDelete('CASCADE')
        .notNullable()
      table.string('zabbix_uuid').nullable()
      table.string('name').notNullable()
      table.string('key').notNullable()
      table.string('snmp_oid').notNullable()
      table.string('value_type').notNullable()
      table.string('units').nullable()
      table.float('multiplier').nullable()

      table.timestamp('created_at').notNullable()

      table.index(['template_id'], 'zabbix_template_items_template_id_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
