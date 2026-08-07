import type { HttpContext } from '@adonisjs/core/http'
import AlertEvent from '#models/alert_event'
import { EventBus, type SystemEvent } from '#modules/events/event_bus'
import { EventRelay } from '#modules/events/event_relay'

/** Intervalo do keep-alive: evita que proxies derrubem a conexão ociosa */
const HEARTBEAT_MS = 25_000

export default class EventsController {
  /** Assinantes SSE ativos neste processo */
  private static subscribers = 0

  async index({ request, response }: HttpContext) {
    const page = Number(request.input('page', 1))
    const limit = Math.min(Number(request.input('limit', 20)), 100)

    const events = await AlertEvent.query()
      .preload('device')
      .preload('monitor')
      .orderBy('createdAt', 'desc')
      .paginate(page, limit)

    return response.ok(events)
  }

  async stream({ request, response }: HttpContext) {
    const rawRes = response.response

    rawRes.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache, no-transform',
      'Connection': 'keep-alive',
      'X-Accel-Buffering': 'no',
    })
    rawRes.flushHeaders?.()

    // Orienta o EventSource a reconectar em 3s caso a conexão caia
    rawRes.write('retry: 3000\n\n')

    const write = (event: SystemEvent) => {
      try {
        // Sem `event:` nomeado de propósito — todos chegam no `onmessage`
        // do EventSource e são despachados pelo campo `type` do payload.
        rawRes.write(`data: ${JSON.stringify(event)}\n\n`)
      } catch {
        // Conexão encerrada
      }
    }

    write({
      type: 'stream:connected',
      timestamp: new Date().toISOString(),
      data: {},
    })

    const eventBus = EventBus.getInstance()
    const unsubscribe = eventBus.subscribe(write)

    // O relay traz os eventos gerados pelo scheduler/worker/probes; só vale a
    // pena consultar a caixa de saída enquanto houver alguém assistindo.
    EventsController.subscribers += 1
    if (EventsController.subscribers === 1) {
      void EventRelay.getInstance().start()
    }

    const heartbeat = setInterval(() => {
      try {
        // Comentário SSE: mantém o socket vivo sem gerar evento no cliente
        rawRes.write(`: keep-alive ${Date.now()}\n\n`)
      } catch {
        // Conexão encerrada
      }
    }, HEARTBEAT_MS)

    let closed = false
    const cleanup = () => {
      if (closed) return
      closed = true
      clearInterval(heartbeat)
      unsubscribe()

      EventsController.subscribers = Math.max(0, EventsController.subscribers - 1)
      if (EventsController.subscribers === 0) {
        EventRelay.getInstance().stop()
      }
    }

    request.request.on('close', cleanup)
    request.request.on('error', cleanup)
  }
}
