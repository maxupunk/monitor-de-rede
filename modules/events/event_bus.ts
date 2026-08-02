export interface SystemEvent {
  type: string
  timestamp: string
  data: Record<string, unknown>
}

export class EventBus {
  private static instance: EventBus
  private listeners: Set<(event: SystemEvent) => void> = new Set()

  static getInstance(): EventBus {
    if (!EventBus.instance) {
      EventBus.instance = new EventBus()
    }
    return EventBus.instance
  }

  subscribe(listener: (event: SystemEvent) => void): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  emit(type: string, data: Record<string, unknown>): void {
    const event: SystemEvent = {
      type,
      timestamp: new Date().toISOString(),
      data,
    }
    for (const listener of this.listeners) {
      try {
        listener(event)
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err)
        console.error(`[EventBus] Erro ao notificar ouvinte: ${msg}`)
      }
    }
  }

  clearListeners(): void {
    this.listeners.clear()
  }
}
