import type { HttpContext } from '@adonisjs/core/http'
import { EventBus, type SystemEvent } from '#modules/events/event_bus'

export default class EventsController {
  async stream({ request, response }: HttpContext) {
    const rawRes = response.response

    rawRes.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    })

    const initialPayload = `data: ${JSON.stringify({ event: 'connected', timestamp: new Date().toISOString() })}\n\n`
    rawRes.write(initialPayload)

    const eventBus = EventBus.getInstance()
    const unsubscribe = eventBus.subscribe((event: SystemEvent) => {
      try {
        rawRes.write(`data: ${JSON.stringify(event)}\n\n`)
      } catch {
        // Conexão encerrada
      }
    })

    request.request.on('close', () => {
      unsubscribe()
    })
  }
}
