import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'device_links'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('source_device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('CASCADE')
        .notNullable()
      table
        .integer('target_device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('CASCADE')
        .notNullable()
      table
        .integer('source_interface_id')
        .unsigned()
        .references('id')
        .inTable('device_interfaces')
        .onDelete('SET NULL')
        .nullable()
      table
        .integer('target_interface_id')
        .unsigned()
        .references('id')
        .inTable('device_interfaces')
        .onDelete('SET NULL')
        .nullable()
      table.string('link_type').notNullable()
      table.string('discovery_method').notNullable()
      table.integer('confidence').defaultTo(100).notNullable()
      table.boolean('confirmed').defaultTo(false).notNullable()
      table.timestamp('last_seen_at').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      /**
       * O enlace é procurado nos dois sentidos (`source→target` e o inverso),
       * daí o índice composto mais o índice isolado no destino.
       */
      table.index(['source_device_id', 'target_device_id'], 'device_links_source_target_index')
      table.index(['target_device_id'], 'device_links_target_device_id_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
