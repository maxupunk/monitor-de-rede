import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'devices'

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
        .integer('network_id')
        .unsigned()
        .references('id')
        .inTable('networks')
        .onDelete('SET NULL')
        .nullable()
      table
        .integer('parent_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('SET NULL')
        .nullable()
      table
        .integer('zabbix_template_id')
        .unsigned()
        .references('id')
        .inTable('zabbix_templates')
        .onDelete('SET NULL')
        .nullable()
      table.string('ip_address').nullable()
      table.string('name').notNullable()
      table.string('type').notNullable()
      table.string('vendor').nullable()
      table.string('model').nullable()
      table.string('serial_number').nullable()
      table.text('description').nullable()
      table.boolean('is_monitored').defaultTo(false)
      table.boolean('snmp_enabled').defaultTo(false)
      table.string('snmp_community').nullable()
      table.string('snmp_version').nullable()
      table.string('status').defaultTo('unknown').notNullable()
      table.timestamp('last_seen_at').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      /**
       * Integridade do IPAM: impede dois dispositivos com o mesmo IP na mesma
       * rede. NULLs são distintos tanto no PostgreSQL quanto no SQLite, então
       * dispositivos sem IP definido não colidem entre si.
       */
      table.unique(['network_id', 'ip_address'], { indexName: 'devices_network_ip_unique' })

      /**
       * O checker SNMP resolve o equipamento por `ip_address = ? OR name = ?`.
       * O UNIQUE acima já cobre buscas por `network_id`, mas não por IP isolado.
       */
      table.index(['ip_address'], 'devices_ip_address_index')
      table.index(['name'], 'devices_name_index')
      table.index(['site_id'], 'devices_site_id_index')
      table.index(['zabbix_template_id'], 'devices_zabbix_template_id_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
