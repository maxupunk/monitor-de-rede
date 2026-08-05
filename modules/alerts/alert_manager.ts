import { DateTime } from 'luxon'
import type Monitor from '#models/monitor'
import Device from '#models/device'
import AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import type { CheckResult } from '#modules/monitoring/contracts/check_result'
import { RuleEvaluator, type AlertRuleCondition } from './rule_evaluator.js'
import { RecoveryManager } from './recovery_manager.js'
import { SilenceManager } from './silence_manager.js'
import { NotificationService } from '#modules/notifications/notification_service'
import { EventBus } from '#modules/events/event_bus'

/**
 * Nomes das métricas produzidas pelos checkers -> chaves usadas nas condições
 * das regras de alerta. Mantém o vocabulário da UI alinhado ao avaliador.
 */
const METRIC_FIELD_MAP: Record<string, string> = {
  latency: 'latencyMs',
  response_time: 'latencyMs',
  packet_loss: 'packetLoss',
  status_code: 'statusCode',
  connect_time: 'connectTimeMs',
  resolution_time: 'resolutionTimeMs',
  if_oper_status: 'ifOperStatus',
  if_speed: 'ifSpeed',
  snmp_uptime: 'snmpUptime',
}

export class AlertManager {
  private evaluator = new RuleEvaluator()
  private recoveryManager = new RecoveryManager()
  private silenceManager = new SilenceManager()
  private notificationService = new NotificationService()
  private eventBus = EventBus.getInstance()

  /**
   * Momento em que cada regra passou a bater continuamente, por monitor.
   * Usado para respeitar `durationSeconds` (tolerância antes de disparar).
   */
  private static pendingSince = new Map<string, number>()

  /** Traduz o CheckResult para o vocabulário avaliado pelas regras */
  buildDataset(monitor: Monitor, result: CheckResult): Record<string, unknown> {
    const dataset: Record<string, unknown> = {
      status: result.status,
      success: result.success,
      durationMs: result.durationMs,
      type: monitor.type,
      latencyMs: null,
    }

    for (const metric of result.metrics || []) {
      const field = METRIC_FIELD_MAP[metric.name]
      if (!field) continue
      // `latency` tem precedência sobre `response_time` quando ambos existem
      if (field === 'latencyMs' && dataset.latencyMs !== null && metric.name === 'response_time') {
        continue
      }
      dataset[field] = metric.value
    }

    // Campos extras publicados no `data` do checker (ex.: statusCode do HTTP)
    for (const [key, value] of Object.entries(result.data || {})) {
      if (dataset[key] === undefined) dataset[key] = value
    }

    return dataset
  }

  async evaluateMonitorResult(monitor: Monitor, result: CheckResult): Promise<void> {
    const rules = await AlertRule.query()
      .where('enabled', true)
      .where((query) => {
        query
          .where('monitorId', monitor.id)
          .orWhere('deviceId', monitor.deviceId)
          .orWhereNull('monitorId')
      })

    const dataset = this.buildDataset(monitor, result)

    let hasTriggeredRule = false

    for (const rule of rules) {
      const isMatch = this.evaluator.evaluate(
        rule.condition as unknown as AlertRuleCondition,
        dataset
      )

      if (!isMatch) {
        AlertManager.pendingSince.delete(this.pendingKey(rule.id, monitor.id))
        continue
      }

      hasTriggeredRule = true
      if (this.hasSustainedCondition(rule, monitor.id)) {
        await this.triggerAlert(rule, monitor, result)
      }
    }

    if (!hasTriggeredRule && result.status === 'up') {
      await this.recoveryManager.resolveAlertsForMonitor(monitor.id)
    }
  }

  private pendingKey(ruleId: number, monitorId: number): string {
    return `${ruleId}:${monitorId}`
  }

  /**
   * Só libera o disparo quando a condição se mantém pelo tempo configurado em
   * `durationSeconds`, evitando alertas por oscilações momentâneas.
   */
  private hasSustainedCondition(rule: AlertRule, monitorId: number): boolean {
    const tolerance = Number(rule.durationSeconds) || 0
    if (tolerance <= 0) return true

    const key = this.pendingKey(rule.id, monitorId)
    const firstSeen = AlertManager.pendingSince.get(key)
    if (firstSeen === undefined) {
      AlertManager.pendingSince.set(key, Date.now())
      return false
    }

    return Date.now() - firstSeen >= tolerance * 1000
  }

  private async triggerAlert(
    rule: AlertRule,
    monitor: Monitor,
    result: CheckResult
  ): Promise<void> {
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
    const device = await Device.find(monitor.deviceId)
    const title = `${rule.name} — ${device?.name ?? monitor.name}`

    const alertEvent = await AlertEvent.create({
      alertRuleId: rule.id,
      deviceId: monitor.deviceId,
      monitorId: monitor.id,
      status: 'active',
      severity: rule.severity,
      startedAt: DateTime.now(),
      message,
      data: { resultData: result.data || {}, title, ruleName: rule.name },
    })

    await this.notificationService.notify({
      title: `🚨 [${rule.severity.toUpperCase()}] ${rule.name}`,
      body: `Monitor #${monitor.id} (${monitor.type}) falhou: ${message}`,
      severity: rule.severity,
      metadata: { alertEventId: alertEvent.id, monitorId: monitor.id, deviceId: monitor.deviceId },
    })

    this.eventBus.emit('alert:triggered', {
      id: alertEvent.id,
      alertEventId: alertEvent.id,
      alertRuleId: rule.id,
      ruleName: rule.name,
      monitorId: monitor.id,
      monitorName: monitor.name,
      deviceId: monitor.deviceId,
      deviceName: device?.name ?? null,
      severity: rule.severity,
      status: alertEvent.status,
      title,
      message,
      startedAt: alertEvent.startedAt.toISO()!,
      createdAt: alertEvent.createdAt?.toISO() ?? alertEvent.startedAt.toISO()!,
    })
  }
}
