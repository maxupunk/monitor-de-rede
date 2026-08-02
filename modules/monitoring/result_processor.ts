import { DateTime } from 'luxon'
import type { CheckResult } from './contracts/check_result.js'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import Device from '#models/device'
import { AlertManager } from '#modules/alerts/alert_manager'
import { EventBus } from '#modules/events/event_bus'

export class ResultProcessor {
  private alertManager = new AlertManager()
  private eventBus = EventBus.getInstance()

  async processResult(monitorId: number, result: CheckResult, probeId?: number | null): Promise<void> {
    const monitor = await Monitor.find(monitorId)
    if (!monitor) return

    const latencyMetric = result.metrics?.find((m) => m.name === 'latency' || m.name === 'response_time')

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

    monitor.status = result.status
    monitor.lastRunAt = finishedAt
    await monitor.save()

    const device = await Device.find(monitor.deviceId)
    if (device) {
      if (result.status === 'up') {
        device.status = 'online'
        device.lastSeenAt = finishedAt
      } else if (result.status === 'down') {
        device.status = 'offline'
      } else if (result.status === 'warning') {
        device.status = 'warning'
      }
      await device.save()
    }

    try {
      await this.alertManager.evaluateMonitorResult(monitor, result)
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[ResultProcessor] Erro ao avaliar alertas do monitor #${monitor.id}: ${msg}`)
    }

    this.eventBus.emit('monitor:result', {
      monitorId: monitor.id,
      deviceId: monitor.deviceId,
      status: result.status,
      latencyMs: latencyMetric ? latencyMetric.value : null,
      finishedAt: finishedAt.toISO()!,
    })
  }
}
