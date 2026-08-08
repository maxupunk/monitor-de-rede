import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'discovery_results'

  async up() {
    // Limpa resultados antigos: discovery_results passa a ser apenas o cache
    // do último scan, sem histórico persistente de status.
    this.schema.raw(`DELETE FROM ${this.tableName}`)

    // Remove a coluna de status: um resultado existe enquanto não foi
    // transformado em device; a verificação de "já adicionado" é feita
    // comparando o IP com a tabela devices.
    this.schema.alterTable(this.tableName, (table) => {
      table.dropColumn('status')
    })
  }

  async down() {
    this.schema.alterTable(this.tableName, (table) => {
      table.string('status').defaultTo('pending').notNullable()
    })
  }
}
