import type { HttpContext } from '@adonisjs/core/http'
import Monitor from '#models/monitor'

export default class MonitorsController {
  async index({ response }: HttpContext) {
    const monitors = await Monitor.all()
    return response.ok(monitors)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only(['deviceId', 'probeId', 'type', 'name', 'configuration', 'intervalSeconds', 'timeoutSeconds', 'retryCount', 'enabled', 'status'])
    const monitor = await Monitor.create(data)
    return response.created(monitor)
  }

  async show({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    return response.ok(monitor)
  }

  async update({ params, request, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    const data = request.only(['deviceId', 'probeId', 'type', 'name', 'configuration', 'intervalSeconds', 'timeoutSeconds', 'retryCount', 'enabled', 'status'])
    monitor.merge(data)
    await monitor.save()
    return response.ok(monitor)
  }

  async destroy({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    await monitor.delete()
    return response.noContent()
  }

  async run({ params, response }: HttpContext) {
    return response.ok({ message: `Execução manual agendada para o monitor ID ${params.id}` })
  }

  async enable({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    monitor.enabled = true
    await monitor.save()
    return response.ok(monitor)
  }

  async disable({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    monitor.enabled = false
    await monitor.save()
    return response.ok(monitor)
  }
}
