import type { HttpContext } from '@adonisjs/core/http'

export default class EventsController {
  async stream({ response }: HttpContext) {
    response.response.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    })
    response.response.write(`data: ${JSON.stringify({ event: 'connected', data: { timestamp: new Date() } })}\n\n`)
  }
}
