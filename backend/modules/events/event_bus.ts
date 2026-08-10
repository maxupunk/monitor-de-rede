import crypto from 'node:crypto'
import EventOutbox from '#models/event_outbox'
import { errorMessage } from '#modules/shared/errors'

export interface SystemEvent {
  type: string
  timestamp: string
  data: Record<string, unknown>
}

/**
 * Identificador único deste processo. O relay usa o valor para ignorar os
 * eventos que o próprio processo gravou, evitando entrega duplicada.
 */
export const PROCESS_ORIGIN = `${process.pid}-${crypto.randomBytes(4).toString('hex')}`

export class EventBus {
  private static instance: EventBus
  private listeners: Set<(event: SystemEvent) => void> = new Set()

  /**
   * Publicação na caixa de saída começa desligada (testes e comandos pontuais
   * não precisam propagar); é ligada no boot dos processos de longa duração.
   */
  private publishToOutbox = false

  /** Escritas na caixa de saída ainda em voo, aguardadas no `flush` */
  private pendingWrites = new Set<Promise<unknown>>()

  static getInstance(): EventBus {
    if (!EventBus.instance) {
      EventBus.instance = new EventBus()
    }
    return EventBus.instance
  }

  /**
   * Habilita a gravação dos eventos na tabela `event_outbox`, permitindo que
   * processos de background (scheduler, worker, probe) alcancem as conexões
   * SSE mantidas pelo processo HTTP.
   */
  enableCrossProcessPublishing(): void {
    this.publishToOutbox = true
  }

  disableCrossProcessPublishing(): void {
    this.publishToOutbox = false
  }

  subscribe(listener: (event: SystemEvent) => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  get listenerCount(): number {
    return this.listeners.size
  }

  emit(type: string, data: Record<string, unknown>): void {
    const event: SystemEvent = {
      type,
      timestamp: new Date().toISOString(),
      data,
    }

    this.dispatch(event)

    if (this.publishToOutbox) {
      // Assíncrono para não bloquear o ciclo de monitoramento que originou o
      // evento, mas rastreado para que `flush` não deixe nada para trás.
      const write = EventOutbox.create({ type, origin: PROCESS_ORIGIN, payload: data })
        .catch((err: unknown) => {
          const msg = errorMessage(err)
          console.error(`[EventBus] Falha ao publicar "${type}" na caixa de saída: ${msg}`)
        })
        .finally(() => {
          this.pendingWrites.delete(write)
        })

      this.pendingWrites.add(write)
    }
  }

  /**
   * Aguarda as publicações pendentes. Sem isso, um processo de vida curta
   * (comandos ace pontuais) pode encerrar antes de gravar o último evento.
   */
  async flush(): Promise<void> {
    while (this.pendingWrites.size > 0) {
      await Promise.allSettled([...this.pendingWrites])
    }
  }

  /** Entrega apenas aos ouvintes locais, sem republicar na caixa de saída */
  dispatch(event: SystemEvent): void {
    for (const listener of this.listeners) {
      try {
        listener(event)
      } catch (err: unknown) {
        const msg = errorMessage(err)
        console.error(`[EventBus] Erro ao notificar ouvinte: ${msg}`)
      }
    }
  }

  clearListeners(): void {
    this.listeners.clear()
  }
}
