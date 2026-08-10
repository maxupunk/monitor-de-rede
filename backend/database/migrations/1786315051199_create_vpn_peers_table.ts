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
      /** Cifrada em repouso com a APP_KEY — ver `VpnPeer.presharedKey`. */
      table.text('preshared_key_encrypted').nullable()
      table.string('device_profile').defaultTo('linux').notNullable()
      table.integer('persistent_keepalive').defaultTo(25).notNullable()
      table.timestamp('last_handshake_at').nullable()
      /**
       * Último ciclo em que o servidor contabilizou bytes novos vindos do peer
       * — na prática, o último keepalive recebido. O handshake sozinho não
       * serve como sinal de vida: o WireGuard só renegocia chaves quando há o
       * que enviar, então um túnel ocioso mas saudável passa vários minutos sem
       * handshake novo.
       */
      table.timestamp('last_seen_at').nullable()
      table.bigInteger('bytes_rx').defaultTo(0).notNullable()
      table.bigInteger('bytes_tx').defaultTo(0).notNullable()
      table.boolean('enabled').defaultTo(true).notNullable()
      /**
       * Memória do ciclo anterior do túnel: alerta nasce de uma *transição*
       * (`connected ➔ disconnected`), e transição exige saber onde o túnel
       * estava antes.
       */
      table.string('last_connection_status').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      table.unique(['device_id'])

      /** Peers ativos de um servidor: lido a cada ciclo de sincronia do WireGuard. */
      table.index(['vpn_server_id', 'enabled'], 'vpn_peers_server_enabled_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
