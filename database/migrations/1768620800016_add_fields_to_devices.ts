import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'devices'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.integer('site_id').nullable().alter()
      table.string('ip_address').nullable()
      table
        .integer('parent_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('SET NULL')
        .nullable()
      table.boolean('is_monitored').defaultTo(false)
      table.boolean('snmp_enabled').defaultTo(false)
      table.string('snmp_community').nullable()
      table.string('snmp_version').nullable()
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropColumn('ip_address')
      table.dropColumn('parent_id')
      table.dropColumn('is_monitored')
      table.dropColumn('snmp_enabled')
      table.dropColumn('snmp_community')
      table.dropColumn('snmp_version')
    })
  }
}
