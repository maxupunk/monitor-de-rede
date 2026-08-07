import { DateTime } from 'luxon'
import ProbeTaskRecord from '#models/probe_task'

/** Contrato de fio entre o servidor e o agente do probe. */
export interface ProbeTask {
  id: string
  monitorId: number
  type: 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'
  timeoutMs: number
  payload: Record<string, unknown>
}

/**
 * Tempo máximo que uma tarefa enfileirada ainda vale.
 *
 * Uma checagem que ficou parada porque o probe estava fora do ar não descreve
 * mais o presente: executá-la produziria um resultado carimbado com a hora
 * errada. Passado esse prazo a tarefa é descartada e o scheduler enfileira uma
 * nova no próximo ciclo.
 */
export const TASK_TTL_SECONDS = 120

/** Teto de tarefas entregues por polling, para não travar um probe que ficou atrás. */
const DELIVERY_BATCH_LIMIT = 100

/**
 * Fila de tarefas dos probes, persistida em `probe_tasks`.
 *
 * Precisa atravessar processos: quem enfileira é o scheduler e quem entrega é o
 * processo HTTP. Ver a migration `create_probe_tasks_table` para o histórico.
 */
export class ProbeTaskDispatcher {
  /**
   * Enfileira a tarefa, substituindo a pendente do mesmo monitor.
   *
   * Sem a substituição, um probe offline acumularia uma tarefa por ciclo e
   * dispararia todas de uma vez ao voltar.
   */
  async dispatchTask(probeId: number | string, task: ProbeTask): Promise<void> {
    await ProbeTaskRecord.query().where('monitorId', task.monitorId).delete()

    await ProbeTaskRecord.create({
      probeId: Number(probeId),
      monitorId: task.monitorId,
      taskId: task.id,
      type: task.type,
      timeoutMs: task.timeoutMs,
      payload: task.payload,
    })
  }

  /** Entrega e remove as tarefas do probe, descartando as que já venceram. */
  async getPendingTasks(probeId: number | string): Promise<ProbeTask[]> {
    const rows = await ProbeTaskRecord.query()
      .where('probeId', Number(probeId))
      .orderBy('id', 'asc')
      .limit(DELIVERY_BATCH_LIMIT)

    if (rows.length === 0) return []

    await ProbeTaskRecord.query()
      .whereIn(
        'id',
        rows.map((row) => row.id)
      )
      .delete()

    const cutoff = DateTime.now().minus({ seconds: TASK_TTL_SECONDS })

    return rows
      .filter((row) => !row.createdAt || row.createdAt > cutoff)
      .map((row) => ({
        id: row.taskId,
        monitorId: row.monitorId,
        type: row.type,
        timeoutMs: row.timeoutMs,
        payload: row.payload,
      }))
  }

  async clearTasksForProbe(probeId: number | string): Promise<void> {
    await ProbeTaskRecord.query().where('probeId', Number(probeId)).delete()
  }
}
