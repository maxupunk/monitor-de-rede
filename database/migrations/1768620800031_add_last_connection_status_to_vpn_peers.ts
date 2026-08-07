import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * `last_connection_status` é a memória do ciclo anterior do túnel.
 *
 * O estado atual (`connectionStatus`) é derivado do último sinal de vida a cada
 * leitura — o que basta para pintar a tela, mas não para alertar: alerta nasce
 * de uma *transição* (`connected ➔ disconnected`), e transição exige saber onde
 * o túnel estava antes. Mesma solução já usada para interfaces SNMP, onde o
 * estado anterior vem de `device_interfaces.oper_status`.
 */
export default class extends BaseSchema {
  protected tableName = 'vpn_peers'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.string('last_connection_status').nullable()
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropColumn('last_connection_status')
    })
  }
}
