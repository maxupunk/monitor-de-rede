import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'metrics'

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
      table
        .integer('interface_id')
        .unsigned()
        .references('id')
        .inTable('device_interfaces')
        .onDelete('SET NULL')
        .nullable()
      table
        .integer('monitor_id')
        .unsigned()
        .references('id')
        .inTable('monitors')
        .onDelete('SET NULL')
        .nullable()
      table.string('name').notNullable()
      table.double('value').notNullable()
      table.string('unit').notNullable()
      table.timestamp('recorded_at').notNullable()

      table.timestamp('created_at').notNullable()

      /**
       * Série temporal com inserção em rajada: cada índice custa em toda coleta
       * SNMP, então são só os quatro que atendem laços quentes.
       *
       * O primeiro serve o "último valor por interface" (tráfego SNMP) e, pelo
       * prefixo `device_id`, também os filtros por equipamento. O segundo serve
       * o "último valor por métrica" (bytes da VPN, sparkline de CPU/memória).
       */
      table.index(
        ['device_id', 'interface_id', 'name', 'recorded_at'],
        'metrics_device_interface_name_recorded_index'
      )
      table.index(['device_id', 'name', 'recorded_at'], 'metrics_device_name_recorded_index')
      table.index(['interface_id', 'recorded_at'], 'metrics_interface_recorded_index')
      table.index(['created_at'], 'metrics_created_at_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
