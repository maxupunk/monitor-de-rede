import { BaseSchema } from '@adonisjs/lucid/schema'

export default class extends BaseSchema {
  protected tableName = 'probes'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.increments('id').notNullable()
      table.integer('site_id').unsigned().references('id').inTable('sites').onDelete('CASCADE').nullable()
      table.string('name').notNullable()
      table.string('token_hash').notNullable()
      table.string('status').defaultTo('pending').notNullable()
      table.string('version').nullable()
      table.timestamp('last_seen_at').nullable()
      table.timestamp('registered_at').nullable()
      table.timestamp('revoked_at').nullable()
      table.jsonb('configuration').nullable()

      table.timestamp('created_at').notNullable()
      table.timestamp('updated_at').nullable()
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
