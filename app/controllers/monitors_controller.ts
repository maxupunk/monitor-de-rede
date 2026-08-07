import type { HttpContext } from '@adonisjs/core/http'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
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

  /**
   * Traz a última leitura e um histórico limitado por monitor de uso (CPU/Memória),
   * para que a lista de monitores (dashboard e /monitors) já abra com um gráfico de
   * tendência em vez de só o valor mais recente — a mesma leitura em tempo real via
   * SSE (`applyRealtimeMetrics` no store) só precisa então anexar a esse histórico.
   */
  private async fetchGaugeMetricsData(monitors: Monitor[], historyLimit = 20) {
    const gaugeMonitors = monitors
      .map((mon) => ({ mon, metricName: this.gaugeMetricName(mon) }))
      .filter(
        (entry): entry is { mon: Monitor; metricName: string } =>
          entry.metricName !== null && entry.mon.deviceId !== null
      )

    const latestMap = new Map<
      number,
      { name: string; value: number; unit: string; recordedAt: string }
    >()
    const historyMap = new Map<number, Array<{ value: number; recordedAt: string }>>()
    if (gaugeMonitors.length === 0) return { latestMap, historyMap }

    // Uma leitura de uso pertence a (equipamento, métrica) — deduplica antes de
    // consultar para não repetir a mesma busca quando dois monitores apontam para
    // o mesmo par (ex: dois monitores de CPU no mesmo equipamento).
    const pairs = new Map<string, { deviceId: number; metricName: string }>()
    for (const { mon, metricName } of gaugeMonitors) {
      pairs.set(`${mon.deviceId}:${metricName}`, { deviceId: mon.deviceId!, metricName })
    }

    const rowsByPair = new Map<string, Metric[]>()
    await Promise.all(
      [...pairs.entries()].map(async ([key, { deviceId, metricName }]) => {
        const rows = await Metric.query()
          .where('deviceId', deviceId)
          .where('name', metricName)
          .orderBy('recordedAt', 'desc')
          .limit(historyLimit)
        rowsByPair.set(key, rows)
      })
    )

    for (const { mon, metricName } of gaugeMonitors) {
      const rows = rowsByPair.get(`${mon.deviceId}:${metricName}`) ?? []
      if (rows.length === 0) continue

      // `rows` chega do mais recente para o mais antigo (orderBy desc) — inverte
      // para o histórico ficar do mais antigo para o mais recente, como o resto da UI espera.
      historyMap.set(
        mon.id,
        [...rows].reverse().map((r) => ({ value: r.value, recordedAt: r.recordedAt.toISO()! }))
      )

      const latest = rows[0]
      latestMap.set(mon.id, {
        name: latest.name,
        value: latest.value,
        unit: latest.unit,
        recordedAt: latest.recordedAt.toISO()!,
      })
    }

    return { latestMap, historyMap }
  }

  async index({ response }: HttpContext) {
    const monitors = await Monitor.query()
      .preload('device')
      .preload('probe')
      // `.limit()` sozinho, com múltiplos monitores sendo carregados na mesma consulta,
      // limita o total combinado entre TODOS os monitores (não por monitor) — poucos
      // monitores "consomem" o limite inteiro e os demais ficam quase sem histórico.
      // `.groupLimit()` usa ROW_NUMBER() OVER (PARTITION BY monitor_id) para trazer até
      // N resultados de cada monitor individualmente. `.groupOrderBy` vai direto para SQL
      // raw (sem passar pela conversão camelCase -> snake_case do Lucid), então precisa do
      // nome da coluna no banco (`started_at`), não da propriedade do model (`startedAt`).
      .preload('results', (query) => query.groupLimit(30).groupOrderBy('started_at', 'desc'))

    const { latestMap, historyMap } = await this.fetchGaugeMetricsData(monitors)

    const formatted = monitors.map((mon) => {
      const json = mon.serialize()
      const results = mon.results || []
      json.recentResults = [...results].reverse().map((r) => r.serialize())
      json.gaugeMetric = latestMap.get(mon.id) || null
      json.gaugeHistory = historyMap.get(mon.id) || []
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

    const { latestMap, historyMap } = await this.fetchGaugeMetricsData([monitor])

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
}
