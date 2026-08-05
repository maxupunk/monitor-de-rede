import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * Nem toda checagem pertence a um equipamento: medir a latência de um servidor
 * DNS público ou a disponibilidade de um site externo não depende de um
 * dispositivo cadastrado. O vínculo passa a ser opcional.
 */
export default class extends BaseSchema {
  protected tableName = 'monitors'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.integer('device_id').unsigned().nullable().alter()
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.integer('device_id').unsigned().notNullable().alter()
    })
  }
}
