import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * `template_key` liga a regra ao item do catálogo que a originou. É a chave de
 * idempotência usada ao aplicar as regras pré-configuradas: uma regra já
 * derivada de um template nunca é recriada.
 */
export default class extends BaseSchema {
  protected tableName = 'alert_rules'

  async up() {
    this.schema.alterTable(this.tableName, (table) => {
      table.string('template_key').nullable()
      table.index(['template_key'], 'alert_rules_template_key_index')
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.dropIndex(['template_key'], 'alert_rules_template_key_index')
      table.dropColumn('template_key')
    })
  }
}
