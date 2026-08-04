import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'vpn_servers'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('network_id')
        .unsigned()
        .references('id')
        .inTable('networks')
        .onDelete('CASCADE')
        .notNullable()
      table.string('interface_name').defaultTo('wg0').notNullable()
      table.integer('listen_port').defaultTo(51820).notNullable()
      table.string('public_endpoint').nullable()
      table.string('public_key').notNullable()
      table.text('private_key_encrypted').notNullable()
      table.boolean('allow_peer_to_peer').defaultTo(false).notNullable()
      table.integer('mtu').defaultTo(1420).notNullable()
      table.string('dns_servers').nullable()
      table.boolean('active').defaultTo(true).notNullable()
      table.timestamp('last_synced_at').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
