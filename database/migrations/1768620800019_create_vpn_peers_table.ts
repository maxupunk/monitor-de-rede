import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'vpn_peers'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table
        .integer('vpn_server_id')
        .unsigned()
        .references('id')
        .inTable('vpn_servers')
        .onDelete('CASCADE')
        .notNullable()
      table
        .integer('device_id')
        .unsigned()
        .references('id')
        .inTable('devices')
        .onDelete('CASCADE')
        .notNullable()
      table.string('public_key').notNullable().unique()
      table.text('preshared_key_encrypted').nullable()
      table.string('device_profile').defaultTo('linux').notNullable()
      table.integer('persistent_keepalive').defaultTo(25).notNullable()
      table.timestamp('last_handshake_at').nullable()
      table.bigInteger('bytes_rx').defaultTo(0).notNullable()
      table.bigInteger('bytes_tx').defaultTo(0).notNullable()
      table.boolean('enabled').defaultTo(true).notNullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      table.unique(['device_id'])
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
