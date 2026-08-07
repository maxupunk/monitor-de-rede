import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * Rastreamento das varreduras periódicas por rede.
 *
 * `scan_enabled` e `scan_interval` já existiam, mas nada os lia: sem saber
 * quando a rede foi varrida pela última vez, o scheduler não tem como decidir
 * quais estão vencidas — o mesmo par `last_run_at` / `next_run_at` que já
 * governa os monitores.
 */
export default class extends BaseSchema {
  protected tableName = 'networks'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.timestamp('last_scan_at').nullable()
      table.timestamp('next_scan_at').nullable()
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropColumn('last_scan_at')
      table.dropColumn('next_scan_at')
    })
  }
}
