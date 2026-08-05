import { DateTime } from 'luxon'
import EventOutbox from '#models/event_outbox'
import { EventBus, PROCESS_ORIGIN, type SystemEvent } from './event_bus.js'
import { errorMessage } from '#modules/shared/errors'

/** Frequência de leitura da caixa de saída */
const POLL_INTERVAL_MS = 1000
/** Retenção das linhas já entregues */
const RETENTION_MINUTES = 10
/** Intervalo entre limpezas da tabela */
const PRUNE_INTERVAL_MS = 60_000
/** Teto de linhas por ciclo, protege contra rajadas de varredura */
const BATCH_LIMIT = 200

/**
 * Traz para dentro deste processo os eventos gravados por outros processos
 * (scheduler, worker, probes) e os reentrega aos ouvintes locais — é o que
 * faz o monitoramento em background chegar às telas via SSE.
 *
 * Só roda enquanto existe alguém escutando: sem clientes SSE conectados,
 * nenhuma consulta é feita.
 */
export class EventRelay {
  private static instance: EventRelay
  private eventBus = EventBus.getInstance()

  private pollTimer: ReturnType<typeof setInterval> | null = null
  private pruneTimer: ReturnType<typeof setInterval> | null = null
  private cursor: number | null = null
  private polling = false

  static getInstance(): EventRelay {
    if (!EventRelay.instance) {
      EventRelay.instance = new EventRelay()
    }
    return EventRelay.instance
  }

  get isRunning(): boolean {
    return this.pollTimer !== null
  }

  async start(): Promise<void> {
    if (this.pollTimer) return

    // Começa do fim da fila: eventos anteriores à conexão não interessam
    if (this.cursor === null) {
      this.cursor = await this.currentMaxId()
    }

    this.pollTimer = setInterval(() => {
      void this.poll()
    }, POLL_INTERVAL_MS)

    this.pruneTimer = setInterval(() => {
      void this.prune()
    }, PRUNE_INTERVAL_MS)
  }

  stop(): void {
    if (this.pollTimer) {
      clearInterval(this.pollTimer)
      this.pollTimer = null
    }
    if (this.pruneTimer) {
      clearInterval(this.pruneTimer)
      this.pruneTimer = null
    }
    // Zera o cursor para que a próxima conexão retome do topo da fila
    this.cursor = null
  }

  private async currentMaxId(): Promise<number> {
    try {
      const latest = await EventOutbox.query().orderBy('id', 'desc').first()
      return latest?.id ?? 0
    } catch {
      return 0
    }
  }

  private async poll(): Promise<void> {
    // Evita ciclos sobrepostos quando uma consulta demora mais que o intervalo
    if (this.polling || this.cursor === null) return
    this.polling = true

    try {
      const rows = await EventOutbox.query()
        .where('id', '>', this.cursor)
        .orderBy('id', 'asc')
        .limit(BATCH_LIMIT)

      for (const row of rows) {
        this.cursor = Math.max(this.cursor, row.id)

        // Eventos gravados por este processo já foram entregues no `emit`
        if (row.origin === PROCESS_ORIGIN) continue

        const event: SystemEvent = {
          type: row.type,
          timestamp: row.createdAt?.toISO() ?? new Date().toISOString(),
          data: row.payload || {},
        }
        this.eventBus.dispatch(event)
      }
    } catch (err: unknown) {
      const msg = errorMessage(err)
      console.error(`[EventRelay] Falha ao ler a caixa de saída de eventos: ${msg}`)
    } finally {
      this.polling = false
    }
  }

  private async prune(): Promise<void> {
    try {
      // Data como objeto JS: o knex converte conforme o dialeto (pg/sqlite)
      await EventOutbox.query()
        .where('createdAt', '<', DateTime.now().minus({ minutes: RETENTION_MINUTES }).toJSDate())
        .delete()
    } catch (err: unknown) {
      const msg = errorMessage(err)
      console.error(`[EventRelay] Falha ao limpar a caixa de saída de eventos: ${msg}`)
    }
  }
}
