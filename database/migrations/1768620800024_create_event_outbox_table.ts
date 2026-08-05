import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * Caixa de saída de eventos: ponte entre os processos que produzem eventos
 * (scheduler, worker, probes) e o processo HTTP que mantém as conexões SSE.
 * O EventBus é um singleton em memória, então sem esta tabela nada que roda
 * em background chega ao navegador.
 */
export default class extends BaseSchema {
  protected tableName = 'event_outbox'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.bigIncrements('id').notNullable()
      table.string('type').notNullable()
      // Identifica o processo emissor para que ele não reprocesse o próprio evento
      table.string('origin').notNullable()
      table.jsonb('payload').notNullable()
      table.timestamp('created_at').notNullable()

      table.index(['created_at'], 'event_outbox_created_at_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
