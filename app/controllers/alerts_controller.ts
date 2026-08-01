import type { HttpContext } from '@adonisjs/core/http'

export default class AlertsController {
  async rulesIndex({ response }: HttpContext) {
    return response.ok([])
  }

  async rulesStore({ request, response }: HttpContext) {
    const data = request.only(['siteId', 'deviceId', 'monitorId', 'name', 'type', 'condition', 'severity', 'enabled'])
    return response.created(data)
  }

  async rulesUpdate({ params, request, response }: HttpContext) {
    const data = request.only(['name', 'condition', 'severity', 'enabled'])
    return response.ok({ id: params.id, ...data })
  }

  async rulesDestroy({ params, response }: HttpContext) {
    return response.ok({ id: params.id, deleted: true })
  }

  async index({ response }: HttpContext) {
    return response.ok([])
  }

  async acknowledge({ params, response }: HttpContext) {
    return response.ok({ message: `Alerta ID ${params.id} reconhecido` })
  }

  async silence({ params, response }: HttpContext) {
    return response.ok({ message: `Alerta ID ${params.id} silenciado` })
  }
}
