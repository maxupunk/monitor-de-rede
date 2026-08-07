import { BaseSchema } from '@adonisjs/lucid/schema'

/**
 * Fila de tarefas dos probes.
 *
 * Mesmo problema — e mesma solução — da `event_outbox`: quem enfileira é o
 * scheduler (`node ace scheduler:run`) e quem entrega é o processo HTTP, que
 * responde ao `GET /api/probes/tasks`. Enquanto a fila viveu num `Map` estático
 * em memória, o scheduler empilhava tarefas no próprio processo e o probe
 * consultava uma fila sempre vazia: nenhum monitor atribuído a probe rodava
 * sozinho, ficando eternamente em `unknown`.
 *
 * `monitor_id` é único: um monitor tem no máximo uma tarefa pendente. Com probe
 * offline a linha é substituída a cada ciclo em vez de acumular, e quando ele
 * volta executa uma checagem atual por monitor — não uma avalanche de tarefas
 * vencidas.
 */
export default class extends BaseSchema {
  protected tableName = 'probe_tasks'

  async up() {
    this.schema.createTable(this.tableName, (table) => {
      table.bigIncrements('id').notNullable()
      table
        .integer('probe_id')
        .unsigned()
        .references('id')
        .inTable('probes')
        .onDelete('CASCADE')
        .notNullable()
      table
        .integer('monitor_id')
        .unsigned()
        .references('id')
        .inTable('monitors')
        .onDelete('CASCADE')
        .notNullable()
        .unique()
      // Identificador que o probe devolve junto do resultado.
      table.string('task_id').notNullable()
      table.string('type').notNullable()
      table.integer('timeout_ms').notNullable()
      table.jsonb('payload').notNullable()
      table.timestamp('created_at').notNullable()

      table.index(['probe_id', 'created_at'], 'probe_tasks_probe_id_created_at_index')
    })
  }

  async down() {
    this.schema.dropTable(this.tableName)
  }
}
