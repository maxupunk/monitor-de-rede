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

  private buildConfiguration(
    type: string,
    inputConfig: Record<string, unknown> | undefined,
    target?: string,
    port?: number
  ): Record<string, unknown> {
    const config: Record<string, unknown> = inputConfig ? { ...inputConfig } : {}
    const normType = (type || 'ping').toLowerCase()

    if (target) {
      if (normType === 'ping' || normType === 'snmp') {
        config.host = config.host || target
      } else if (normType === 'http' || normType === 'https') {
        config.url = config.url || (target.startsWith('http') ? target : `http://${target}`)
      } else if (normType === 'tcp') {
        config.host = config.host || target
        if (port) config.port = port
      } else if (normType === 'dns') {
        config.domain = config.domain || target
      }
    }

    if (port && config.port === undefined) {
      config.port = port
    }

    return config
  }

  async store({ request, response }: HttpContext) {
    const rawData = request.only([
      'deviceId',
      'probeId',
      'type',
      'name',
      'configuration',
      'target',
      'port',
      'intervalSeconds',
      'timeoutSeconds',
      'retryCount',
      'enabled',
      'isEnabled',
      'status',
    ])

    const configuration = this.buildConfiguration(
      rawData.type,
      rawData.configuration as Record<string, unknown> | undefined,
      rawData.target as string | undefined,
      rawData.port as number | undefined
    )

    const enabled = rawData.enabled ?? rawData.isEnabled ?? true

    const data = {
      deviceId: rawData.deviceId,
      probeId: rawData.probeId,
      type: rawData.type,
      name: rawData.name,
      configuration,
      intervalSeconds: rawData.intervalSeconds ?? 60,
      timeoutSeconds: rawData.timeoutSeconds ?? 10,
      retryCount: rawData.retryCount ?? 3,
      enabled,
      status: rawData.status ?? 'unknown',
    }

    const monitor = await Monitor.create(data)
    return response.created(monitor)
  }

  async show({ params, response }: HttpContext) {
    const monitor = await Monitor.query().where('id', params.id).preload('device').preload('probe').firstOrFail()
    return response.ok(monitor)
  }

  async update({ params, request, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    const rawData = request.only([
      'deviceId',
      'probeId',
      'type',
      'name',
      'configuration',
      'target',
      'port',
      'intervalSeconds',
      'timeoutSeconds',
      'retryCount',
      'enabled',
      'isEnabled',
      'status',
    ])

    const type = rawData.type || monitor.type
    const configuration = this.buildConfiguration(
      type,
      (rawData.configuration || monitor.configuration) as Record<string, unknown>,
      rawData.target as string | undefined,
      rawData.port as number | undefined
    )

    const data: Record<string, unknown> = {}
    if (rawData.deviceId !== undefined) data.deviceId = rawData.deviceId
    if (rawData.probeId !== undefined) data.probeId = rawData.probeId
    if (rawData.type !== undefined) data.type = rawData.type
    if (rawData.name !== undefined) data.name = rawData.name
    data.configuration = configuration
    if (rawData.intervalSeconds !== undefined) data.intervalSeconds = rawData.intervalSeconds
    if (rawData.timeoutSeconds !== undefined) data.timeoutSeconds = rawData.timeoutSeconds
    if (rawData.retryCount !== undefined) data.retryCount = rawData.retryCount
    const enabled = rawData.enabled ?? rawData.isEnabled
    if (enabled !== undefined) data.enabled = enabled
    if (rawData.status !== undefined) data.status = rawData.status

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
