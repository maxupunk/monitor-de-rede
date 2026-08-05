import type { HttpContext } from '@adonisjs/core/http'
import Monitor from '#models/monitor'
import Metric from '#models/metric'
import Device from '#models/device'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'
import { DeviceStatusService } from '#modules/monitoring/device_status_service'
import { RecoveryManager } from '#modules/alerts/recovery_manager'
import { ResourceCleanupService } from '#services/resource_cleanup_service'

const GAUGE_METRIC_NAMES = ['cpu_usage', 'memory_usage']

export default class MonitorsController {
  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()
  private recoveryManager = new RecoveryManager()

  /**
   * Monitores SNMP de uso de CPU/Memória não são checagens de disponibilidade
   * (up/down) e sim leituras de percentual de uso — identifica esse tipo aqui
   * para que possamos anexar a última leitura em vez de um status up/down.
   */
  private gaugeMetricName(monitor: Monitor): string | null {
    if (monitor.type !== 'snmp') return null
    const metric = (monitor.configuration as Record<string, unknown> | null)?.metric
    return typeof metric === 'string' && GAUGE_METRIC_NAMES.includes(metric) ? metric : null
  }

  private async fetchLatestGaugeMetrics(monitors: Monitor[]) {
    const gaugeMonitors = monitors
      .map((mon) => ({ mon, metricName: this.gaugeMetricName(mon) }))
      .filter((entry): entry is { mon: Monitor; metricName: string } => entry.metricName !== null)

    const map = new Map<number, { name: string; value: number; unit: string; recordedAt: string }>()
    if (gaugeMonitors.length === 0) return map

    // Leituras de uso sempre pertencem a um equipamento; monitores sem vínculo
    // (checagens externas) não têm métrica de gauge para buscar.
    const deviceIds = [
      ...new Set(
        gaugeMonitors
          .map((entry) => entry.mon.deviceId)
          .filter((deviceId): deviceId is number => deviceId !== null)
      ),
    ]
    if (deviceIds.length === 0) return map

    const metricNames = [...new Set(gaugeMonitors.map((entry) => entry.metricName))]

    const rows = await Metric.query()
      .whereIn('deviceId', deviceIds)
      .whereIn('name', metricNames)
      .orderBy('recordedAt', 'desc')

    const latestByDeviceMetric = new Map<string, Metric>()
    for (const row of rows) {
      const key = `${row.deviceId}:${row.name}`
      if (!latestByDeviceMetric.has(key)) latestByDeviceMetric.set(key, row)
    }

    for (const { mon, metricName } of gaugeMonitors) {
      const row = latestByDeviceMetric.get(`${mon.deviceId}:${metricName}`)
      if (row) {
        map.set(mon.id, {
          name: row.name,
          value: row.value,
          unit: row.unit,
          recordedAt: row.recordedAt.toISO()!,
        })
      }
    }

    return map
  }

  async index({ response }: HttpContext) {
    const monitors = await Monitor.query()
      .preload('device')
      .preload('probe')
      .preload('results', (query) => query.orderBy('startedAt', 'desc').limit(30))

    const gaugeMetrics = await this.fetchLatestGaugeMetrics(monitors)

    const formatted = monitors.map((mon) => {
      const json = mon.serialize()
      const results = mon.results || []
      json.recentResults = [...results].reverse().map((r) => r.serialize())
      json.gaugeMetric = gaugeMetrics.get(mon.id) || null
      return json
    })

    return response.ok(formatted)
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

    const gaugeMetrics = await this.fetchLatestGaugeMetrics([monitor])

    const json = monitor.serialize()
    json.recentResults = [...results].reverse().map((r) => r.serialize())
    json.gaugeMetric = gaugeMetrics.get(monitor.id) || null
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
}
