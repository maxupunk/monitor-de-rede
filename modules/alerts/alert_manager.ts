import { DateTime } from 'luxon'
import Monitor from '#models/monitor'
import AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import type { CheckResult } from '#modules/monitoring/contracts/check_result'
import { RuleEvaluator, type AlertRuleCondition } from './rule_evaluator.js'
import { RecoveryManager } from './recovery_manager.js'
import { SilenceManager } from './silence_manager.js'
import { NotificationService } from '#modules/notifications/notification_service'
import { EventBus } from '#modules/events/event_bus'

export class AlertManager {
  private evaluator = new RuleEvaluator()
  private recoveryManager = new RecoveryManager()
  private silenceManager = new SilenceManager()
  private notificationService = new NotificationService()
  private eventBus = EventBus.getInstance()

  async evaluateMonitorResult(monitor: Monitor, result: CheckResult): Promise<void> {
    const rules = await AlertRule.query()
      .where('enabled', true)
      .where((query) => {
        query.where('monitorId', monitor.id).orWhere('deviceId', monitor.deviceId).orWhereNull('monitorId')
      })

    const latencyMetric = result.metrics?.find((m) => m.name === 'latency' || m.name === 'response_time')

    const dataset: Record<string, unknown> = {
      status: result.status,
      success: result.success,
      durationMs: result.durationMs,
      latencyMs: latencyMetric ? latencyMetric.value : null,
      type: monitor.type,
    }

    let hasTriggeredRule = false

    for (const rule of rules) {
      const isMatch = this.evaluator.evaluate(rule.condition as unknown as AlertRuleCondition, dataset)
      if (isMatch) {
        hasTriggeredRule = true
        await this.triggerAlert(rule, monitor, result)
      }
    }

    if (!hasTriggeredRule && result.status === 'up') {
      await this.recoveryManager.resolveAlertsForMonitor(monitor.id)
    }
  }

  private async triggerAlert(rule: AlertRule, monitor: Monitor, result: CheckResult): Promise<void> {
    const existingActive = await AlertEvent.query()
      .where('alertRuleId', rule.id)
      .where('monitorId', monitor.id)
      .whereIn('status', ['active', 'acknowledged', 'silenced'])
      .first()

    if (existingActive) {
      if (this.silenceManager.isSilenced(existingActive)) {
        return
      }
      return
    }

    const message = result.message || `Alerta disparado pela regra: ${rule.name}`

    const alertEvent = await AlertEvent.create({
      alertRuleId: rule.id,
      deviceId: monitor.deviceId,
      monitorId: monitor.id,
      status: 'active',
      severity: rule.severity,
      startedAt: DateTime.now(),
      message,
      data: { resultData: result.data || {} },
    })

    await this.notificationService.notify({
      title: `🚨 [${rule.severity.toUpperCase()}] ${rule.name}`,
      body: `Monitor #${monitor.id} (${monitor.type}) falhou: ${message}`,
      severity: rule.severity,
      metadata: { alertEventId: alertEvent.id, monitorId: monitor.id, deviceId: monitor.deviceId },
    })

    this.eventBus.emit('alert:triggered', {
      alertEventId: alertEvent.id,
      alertRuleId: rule.id,
      monitorId: monitor.id,
      deviceId: monitor.deviceId,
      severity: rule.severity,
      message,
      startedAt: alertEvent.startedAt.toISO()!,
    })
  }
}
