import { DateTime } from 'luxon'
import type { CheckResult } from './contracts/check_result.js'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import Device from '#models/device'

export class ResultProcessor {
  async processResult(monitorId: number, result: CheckResult, probeId?: number | null): Promise<void> {
    const monitor = await Monitor.find(monitorId)
    if (!monitor) return

    const latencyMetric = result.metrics?.find((m) => m.name === 'latency' || m.name === 'response_time')

    await MonitorResult.create({
      monitorId: monitor.id,
      probeId: probeId ?? monitor.probeId,
      status: result.status,
      startedAt: DateTime.fromJSDate(result.startedAt),
      finishedAt: DateTime.fromJSDate(result.finishedAt),
      durationMs: Math.round(result.durationMs),
      latencyMs: latencyMetric ? latencyMetric.value : null,
      message: result.message || null,
      data: result.data || {},
    })

    monitor.status = result.status
    monitor.lastRunAt = DateTime.fromJSDate(result.finishedAt)
    await monitor.save()

    const device = await Device.find(monitor.deviceId)
    if (device) {
      if (result.status === 'up') {
        device.status = 'online'
        device.lastSeenAt = DateTime.fromJSDate(result.finishedAt)
      } else if (result.status === 'down') {
        device.status = 'offline'
      } else if (result.status === 'warning') {
        device.status = 'warning'
      }
      await device.save()
    }
  }
}
