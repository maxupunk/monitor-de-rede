import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * `last_seen_at` registra o último ciclo em que o servidor contabilizou bytes
 * novos vindos do peer — na prática, o último keepalive que chegou.
 *
 * O handshake sozinho não serve como sinal de vida: o WireGuard só renegocia
 * chaves quando há o que enviar, então um túnel ocioso mas saudável passa
 * vários minutos sem handshake novo e era classificado como "instável".
 */
export default class extends BaseSchema {
  protected tableName = 'vpn_peers'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.timestamp('last_seen_at').nullable()
    })

    // Peers já existentes: o handshake conhecido é a melhor aproximação do
    // último sinal de vida até o primeiro ciclo de sincronização.
    this.defer(async (db) => {
      await db
        .from(this.tableName)
        .whereNotNull('last_handshake_at')
        .update({ last_seen_at: db.raw('last_handshake_at') })
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropColumn('last_seen_at')
    })
  }
}
