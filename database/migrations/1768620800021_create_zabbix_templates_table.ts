import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'zabbix_templates'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table.string('zabbix_uuid').nullable()
      table.string('name').notNullable()
      table.text('description').nullable()
      table.string('zabbix_version').nullable()
      table.jsonb('raw_export').notNullable()

      table.timestamp('imported_at').notNullable()
      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').notNullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
