import { DateTime } from 'luxon'
import type Monitor from '#models/monitor'
import Device from '#models/device'
import type AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import type { CheckResult } from '#modules/monitoring/contracts/check_result'
import { RuleEvaluator, type AlertRuleCondition } from './rule_evaluator.js'
import { RecoveryManager } from './recovery_manager.js'
import { AlertRuleRepository } from './alert_rule_repository.js'
import { buildMonitorResultDataset } from './datasets/monitor_result_dataset.js'
import { AlertScopeKey, type AlertEvaluationContext } from './contracts/alert_evaluation.js'
import { NotificationService } from '#modules/notifications/notification_service'
import { EventBus } from '#modules/events/event_bus'

/**
 * Motor de alertas.
 *
 * Recebe *fatos* já traduzidos para o vocabulário das regras e decide o que
 * vira alerta. Toda política (o que é grave, quanto tolerar) mora nas regras
 * cadastradas — acrescentar um novo tipo de observação não exige tocar aqui,
 * basta publicar um dataset com os campos correspondentes.
 */
export class AlertManager {
  private evaluator = new RuleEvaluator()
  private rules = new AlertRuleRepository()
  private recoveryManager = new RecoveryManager()
  private notificationService = new NotificationService()
  private eventBus = EventBus.getInstance()

  /**
   * Momento em que cada regra passou a bater continuamente, por alvo.
   * Usado para respeitar `durationSeconds` (tolerância antes de disparar).
   */
  private static pendingSince = new Map<string, number>()

  /** Avalia um conjunto de fatos contra as regras aplicáveis ao alvo. */
  async evaluate(context: AlertEvaluationContext): Promise<void> {
    const rules = await this.rules.findEnabledForScope(context.scope)
    let hasTriggeredRule = false

    for (const rule of rules) {
      const isMatch = this.evaluator.evaluate(
        rule.condition as unknown as AlertRuleCondition,
        context.dataset
      )

      if (!isMatch) {
        AlertManager.pendingSince.delete(this.pendingKey(rule.id, context.scopeKey))
        continue
      }

      hasTriggeredRule = true
      if (this.hasSustainedCondition(rule, context.scopeKey)) {
        await this.triggerAlert(rule, context)
      }
    }

    if (!hasTriggeredRule && context.recovered) {
      await this.recoveryManager.resolveScope(context.scopeKey)
    }
  }

  /** Adapta o resultado de um monitor ao contrato genérico de avaliação. */
  async evaluateMonitorResult(monitor: Monitor, result: CheckResult): Promise<void> {
    const device = await Device.find(monitor.deviceId)

    await this.evaluate({
      scope: {
        siteId: device?.siteId ?? null,
        deviceId: monitor.deviceId,
        monitorId: monitor.id,
      },
      scopeKey: AlertScopeKey.monitor(monitor.id),
      targetLabel: device?.name ?? monitor.name,
      dataset: buildMonitorResultDataset(monitor, result),
      message: result.message || null,
      data: { resultData: result.data || {}, monitorType: monitor.type },
      recovered: result.status === 'up',
    })
  }

  private pendingKey(ruleId: number, scopeKey: string): string {
    return `${ruleId}:${scopeKey}`
  }

  /**
   * Só libera o disparo quando a condição se mantém pelo tempo configurado em
   * `durationSeconds`, evitando alertas por oscilações momentâneas.
   */
  private hasSustainedCondition(rule: AlertRule, scopeKey: string): boolean {
    const tolerance = Number(rule.durationSeconds) || 0
    if (tolerance <= 0) return true

    const key = this.pendingKey(rule.id, scopeKey)
    const firstSeen = AlertManager.pendingSince.get(key)
    if (firstSeen === undefined) {
      AlertManager.pendingSince.set(key, Date.now())
      return false
    }

    return Date.now() - firstSeen >= tolerance * 1000
  }

  private async triggerAlert(rule: AlertRule, context: AlertEvaluationContext): Promise<void> {
    // Um alerta aberto por regra e alvo: enquanto não for resolvido, novas
    // ocorrências não geram evento nem notificação repetida.
    const existingActive = await AlertEvent.query()
      .where('alertRuleId', rule.id)
      .where('scopeKey', context.scopeKey)
      .whereIn('status', ['active', 'acknowledged', 'silenced'])
      .first()

    if (existingActive) return

    const message = context.message || `Alerta disparado pela regra: ${rule.name}`
    const title = `${rule.name} — ${context.targetLabel}`

    const alertEvent = await AlertEvent.create({
      alertRuleId: rule.id,
      deviceId: context.scope.deviceId,
      monitorId: context.scope.monitorId,
      scopeKey: context.scopeKey,
      status: 'active',
      severity: rule.severity,
      startedAt: DateTime.now(),
      message,
      data: { ...(context.data ?? {}), title, ruleName: rule.name },
    })

    await this.notificationService.notify({
      title: `🚨 [${rule.severity.toUpperCase()}] ${rule.name}`,
      body: `${context.targetLabel}: ${message}`,
      severity: rule.severity,
      metadata: {
        alertEventId: alertEvent.id,
        monitorId: context.scope.monitorId,
        deviceId: context.scope.deviceId,
        scopeKey: context.scopeKey,
      },
    })

    this.eventBus.emit('alert:triggered', {
      id: alertEvent.id,
      alertEventId: alertEvent.id,
      alertRuleId: rule.id,
      ruleName: rule.name,
      scopeKey: context.scopeKey,
      monitorId: context.scope.monitorId,
      deviceId: context.scope.deviceId,
      targetLabel: context.targetLabel,
      severity: rule.severity,
      status: alertEvent.status,
      title,
      message,
      startedAt: alertEvent.startedAt.toISO()!,
      createdAt: alertEvent.createdAt?.toISO() ?? alertEvent.startedAt.toISO()!,
    })
  }
}
