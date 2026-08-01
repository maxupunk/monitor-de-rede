import type { HttpContext } from '@adonisjs/core/http'
import Monitor from '#models/monitor'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'

export default class MonitorsController {
  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()

  async index({ response }: HttpContext) {
    const monitors = await Monitor.query().preload('device').preload('probe')
    return response.ok(monitors)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only([
      'deviceId',
      'probeId',
      'type',
      'name',
      'configuration',
      'intervalSeconds',
      'timeoutSeconds',
      'retryCount',
      'enabled',
      'status',
    ])
    const monitor = await Monitor.create(data)
    return response.created(monitor)
  }

  async show({ params, response }: HttpContext) {
    const monitor = await Monitor.query().where('id', params.id).preload('device').preload('probe').firstOrFail()
    return response.ok(monitor)
  }

  async update({ params, request, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    const data = request.only([
      'deviceId',
      'probeId',
      'type',
      'name',
      'configuration',
      'intervalSeconds',
      'timeoutSeconds',
      'retryCount',
      'enabled',
      'status',
    ])
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
    const monitor = await Monitor.findOrFail(params.id)
    const result = await this.monitorRunner.runMonitor(monitor.type, monitor.configuration)
    await this.resultProcessor.processResult(monitor.id, result, monitor.probeId)

    return response.ok({
      message: `Execução manual do monitor #${monitor.id} concluída com sucesso`,
      result,
    })
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
