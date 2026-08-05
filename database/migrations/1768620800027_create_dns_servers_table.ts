import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'dns_servers'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table.string('name').notNullable()
      /** IP, `ip:porta` (UDP/TCP) ou endpoint https (DoH) */
      table.string('address').notNullable()
      table.string('protocol').defaultTo('udp').notNullable()
      /** Participa da comparação de latência exibida no dashboard */
      table.boolean('is_default').defaultTo(true).notNullable()
      table.string('description').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()

      table.unique(['address', 'protocol'])
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
