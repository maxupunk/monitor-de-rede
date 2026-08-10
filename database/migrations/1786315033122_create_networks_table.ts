import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'networks'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      /**
       * Opcional: uma sub-rede pode ser cadastrada antes de existir um Site.
       * Exigir o vínculo obrigava a inventar um Site só para poder varrer uma
       * faixa — ver `VpnServerService.resolveNetwork`.
       */
      table
        .integer('site_id')
        .unsigned()
        .references('id')
        .inTable('sites')
        .onDelete('CASCADE')
        .nullable()
      table
        .integer('probe_id')
        .unsigned()
        .references('id')
        .inTable('probes')
        .onDelete('SET NULL')
        .nullable()
      table.string('name').notNullable()
      table.string('cidr').notNullable()
      table.string('gateway').nullable()
      table.integer('vlan').nullable()
      table.jsonb('dns_servers').nullable()
      table.boolean('scan_enabled').defaultTo(true).notNullable()
      table.integer('scan_interval').defaultTo(3600).notNullable()
      table.boolean('active').defaultTo(true).notNullable()

      /**
       * Rastreamento das varreduras periódicas: sem saber quando a rede foi
       * varrida pela última vez o scheduler não tem como decidir quais estão
       * vencidas — mesmo par `last_run_at` / `next_run_at` dos monitores.
       */
      table.timestamp('last_scan_at').nullable()
      table.timestamp('next_scan_at').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
