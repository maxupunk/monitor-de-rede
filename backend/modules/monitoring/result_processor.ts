import { DateTime } from 'luxon'
import type { CheckMetric, CheckResult } from './contracts/check_result.js'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import Device from '#models/device'
import { AlertManager } from '#modules/alerts/alert_manager'
import { EventBus } from '#modules/events/event_bus'
import { DeviceStatusService, type DeviceStatus } from './device_status_service.js'
import { errorMessage } from '#modules/shared/errors'

/**
 * Leitura que um único resultado sugere para o dispositivo. Serve de reserva
 * quando não há monitor habilitado para consolidar; `unknown` e `disabled` não
 * traduzem disponibilidade e por isso ficam de fora.
 */
const OBSERVED_DEVICE_STATUS: Record<string, DeviceStatus | undefined> = {
  up: 'online',
  down: 'offline',
  warning: 'warning',
}

/**
 * Métrica de cada checker que representa o tempo de resposta exibido nos
 * gráficos e KPIs, em ordem de precedência. Sem DNS e TCP nesta lista o
 * `latencyMs` desses monitores ficava nulo e a linha do tempo saía vazia.
 */
const LATENCY_METRIC_PRECEDENCE = [
  'latency',
  'response_time',
  'dns_lookup_time',
  'resolution_time',
  'connect_time',
]

export function pickLatencyMetric(metrics: CheckMetric[] | undefined): CheckMetric | undefined {
  for (const name of LATENCY_METRIC_PRECEDENCE) {
    const metric = metrics?.find((candidate) => candidate.name === name)
    if (metric) return metric
  }
  return undefined
}

export class ResultProcessor {
  private alertManager = new AlertManager()
  private deviceStatusService = new DeviceStatusService()
  private eventBus = EventBus.getInstance()

  async processResult(
    monitorId: number,
    result: CheckResult,
    probeId?: number | null
  ): Promise<void> {
    const monitor = await Monitor.find(monitorId)
    if (!monitor) return

    const latencyMetric = pickLatencyMetric(result.metrics)

    const parseDate = (val: unknown): DateTime => {
      if (val instanceof Date) return DateTime.fromJSDate(val)
      if (typeof val === 'string') return DateTime.fromISO(val)
      return DateTime.now()
    }

    const startedAt = parseDate(result.startedAt)
    const finishedAt = parseDate(result.finishedAt)

    await MonitorResult.create({
      monitorId: monitor.id,
      probeId: probeId ?? monitor.probeId,
      status: result.status,
      startedAt,
      finishedAt,
      durationMs: Math.round(result.durationMs),
      latencyMs: latencyMetric ? latencyMetric.value : null,
      message: result.message || null,
      data: result.data || {},
    })

    const previousMonitorStatus = monitor.status
    monitor.status = result.status
    monitor.lastRunAt = finishedAt
    await monitor.save()

    const device = monitor.deviceId ? await Device.find(monitor.deviceId) : null
    if (device) {
      // Consolida todos os monitores do dispositivo (o status deste já foi
      // gravado acima). O próprio serviço decide se houve transição e só então
      // publica `device:status`.
      await this.deviceStatusService.refreshFromMonitors(device, {
        seenAt: result.status === 'up' ? finishedAt : null,
        observedStatus: OBSERVED_DEVICE_STATUS[result.status],
      })
    }

    try {
      await this.alertManager.evaluateMonitorResult(monitor, result)
    } catch (err: unknown) {
      const msg = errorMessage(err)
      console.error(`[ResultProcessor] Erro ao avaliar alertas do monitor #${monitor.id}: ${msg}`)
    }

    this.eventBus.emit('monitor:result', {
      monitorId: monitor.id,
      id: monitor.id,
      name: monitor.name,
      type: monitor.type,
      deviceId: monitor.deviceId,
      deviceName: device?.name ?? null,
      status: result.status,
      previousStatus: previousMonitorStatus,
      statusChanged: previousMonitorStatus !== result.status,
      latencyMs: latencyMetric ? latencyMetric.value : null,
      durationMs: Math.round(result.durationMs),
      message: result.message || null,
      startedAt: startedAt.toISO()!,
      finishedAt: finishedAt.toISO()!,
    })
  }
}
