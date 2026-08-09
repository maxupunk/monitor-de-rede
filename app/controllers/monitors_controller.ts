import type { HttpContext } from '@adonisjs/core/http'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import Device from '#models/device'
import AlertEvent from '#models/alert_event'
import {
  fetchGaugeMetricsData,
  monitorListQuery,
  presentMonitors,
} from '#modules/monitoring/monitor_presenter'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'
import { DeviceStatusService } from '#modules/monitoring/device_status_service'
import { RecoveryManager } from '#modules/alerts/recovery_manager'
import { ResourceCleanupService } from '#services/resource_cleanup_service'

export default class MonitorsController {
  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()
  private recoveryManager = new RecoveryManager()

  async index({ response }: HttpContext) {
    const monitors = await monitorListQuery().preload('device').preload('probe')
    return response.ok(await presentMonitors(monitors))
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
      intervalSeconds: rawData.intervalSeconds ?? 15,
      timeoutSeconds: rawData.timeoutSeconds ?? 10,
      retryCount: rawData.retryCount ?? 3,
      enabled,
      status: rawData.status ?? 'unknown',
    }

    const monitor = await Monitor.create(data)
    return response.created(monitor)
  }

  async show({ params, response }: HttpContext) {
    const monitor = await Monitor.query()
      .where('id', params.id)
      .preload('device')
      .preload('probe')
      .preload('results', (query) => query.orderBy('startedAt', 'desc').limit(100))
      .firstOrFail()

    const results = monitor.results || []
    const latencies = results
      .map((r) => r.latencyMs)
      .filter((l): l is number => l !== null && l !== undefined)

    const avgLatency =
      latencies.length > 0
        ? Math.round(latencies.reduce((a, b) => a + b, 0) / latencies.length)
        : null
    const minLatency = latencies.length > 0 ? Math.min(...latencies) : null
    const maxLatency = latencies.length > 0 ? Math.max(...latencies) : null
    const lastLatency = latencies.length > 0 ? latencies[0] : null

    const totalChecks = results.length
    const upChecks = results.filter((r) => r.status === 'up').length
    const uptimePercentage =
      totalChecks > 0 ? Number(((upChecks / totalChecks) * 100).toFixed(1)) : 100

    const { latestMap, historyMap } = await fetchGaugeMetricsData([monitor])

    const json = monitor.serialize()
    json.recentResults = [...results].reverse().map((r) => r.serialize())
    json.gaugeMetric = latestMap.get(monitor.id) || null
    json.gaugeHistory = historyMap.get(monitor.id) || []
    json.stats = {
      avgLatency,
      minLatency,
      maxLatency,
      lastLatency,
      uptimePercentage,
      totalChecks,
      upChecks,
    }

    return response.ok(json)
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
    if (enabled === false) {
      await this.recoveryManager.resolveAlertsForMonitor(monitor.id, 'Monitor desativado')
    }
    return response.ok(monitor)
  }

  private cleanupService = new ResourceCleanupService()

  async destroy({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    await this.recoveryManager.resolveAlertsForMonitor(monitor.id, 'Monitor removido')
    await this.cleanupService.deleteMonitor(monitor.id)
    return response.noContent()
  }

  async run({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    const result = await this.monitorRunner.runMonitor(monitor.type, monitor.configuration, {
      timeoutMs: (monitor.timeoutSeconds || 5) * 1000,
    })
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

    const device = monitor.deviceId ? await Device.find(monitor.deviceId) : null
    if (device) {
      await new DeviceStatusService().refreshFromMonitors(device)
    }

    return response.ok(monitor)
  }

  async disable({ params, response }: HttpContext) {
    const monitor = await Monitor.findOrFail(params.id)
    monitor.enabled = false
    await monitor.save()

    await this.recoveryManager.resolveAlertsForMonitor(monitor.id, 'Monitor desativado')

    const device = monitor.deviceId ? await Device.find(monitor.deviceId) : null
    if (device) {
      await new DeviceStatusService().refreshFromMonitors(device)
    }

    return response.ok(monitor)
  }

  async results({ params, request, response }: HttpContext) {
    const page = Number(request.input('page', 1))
    const limit = Math.min(Number(request.input('limit', 20)), 100)

    const results = await MonitorResult.query()
      .where('monitorId', params.id)
      .orderBy('startedAt', 'desc')
      .paginate(page, limit)

    return response.ok(results)
  }

  /**
   * GET /api/monitors/:id/alerts
   * Histórico de alertas vinculados a este monitor (incluindo resolvidos).
   */
  async alerts({ params, request, response }: HttpContext) {
    const page = Number(request.input('page', 1))
    const limit = Math.min(Number(request.input('limit', 20)), 100)

    const events = await AlertEvent.query()
      .where('monitorId', params.id)
      .orWhere('scopeKey', `monitor:${params.id}`)
      .preload('alertRule')
      .preload('device')
      .preload('monitor')
      .orderBy('createdAt', 'desc')
      .paginate(page, limit)

    const data = events.all().map((event) => ({
      id: event.id,
      alertRuleId: event.alertRuleId,
      deviceId: event.deviceId,
      monitorId: event.monitorId,
      scopeKey: event.scopeKey,
      status: event.status,
      severity: event.severity,
      message: event.message,
      data: event.data,
      startedAt: event.startedAt?.toISO() ?? null,
      resolvedAt: event.resolvedAt?.toISO() ?? null,
      createdAt: event.createdAt?.toISO() ?? null,
      updatedAt: event.updatedAt?.toISO() ?? null,
      title:
        (event.data?.title as string | undefined) ||
        [event.alertRule?.name, event.device?.name ?? event.monitor?.name]
          .filter(Boolean)
          .join(' — ') ||
        'Alerta do sistema',
      alertRule: event.alertRule ? { id: event.alertRule.id, name: event.alertRule.name } : null,
      device: event.device ? { id: event.device.id, name: event.device.name } : null,
      monitor: event.monitor ? { id: event.monitor.id, name: event.monitor.name } : null,
      silencedUntil: (event.data?.silencedUntil as string | undefined) ?? null,
    }))

    return response.ok({ data, meta: events.toJSON().meta })
  }
}
