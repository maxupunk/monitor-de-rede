import AlertRule from '#models/alert_rule'
import type { AlertEvaluationScope } from './contracts/alert_evaluation.js'

/**
 * Acesso às regras de alerta. Isola a semântica de escopo (`null` = vale para
 * todo mundo) do restante do motor, que só precisa saber *quais* regras avaliar.
 */
export class AlertRuleRepository {
  /**
   * Regras habilitadas aplicáveis ao alvo. Cada dimensão (site, dispositivo,
   * monitor) é filtrada de forma independente: a regra vale quando não delimita
   * aquela dimensão ou quando aponta exatamente para o alvo avaliado.
   */
  async findEnabledForScope(scope: AlertEvaluationScope): Promise<AlertRule[]> {
    return AlertRule.query()
      .where('enabled', true)
      .where((query) => {
        query.whereNull('siteId')
        if (scope.siteId) query.orWhere('siteId', scope.siteId)
      })
      .where((query) => {
        query.whereNull('deviceId')
        if (scope.deviceId) query.orWhere('deviceId', scope.deviceId)
      })
      .where((query) => {
        query.whereNull('monitorId')
        if (scope.monitorId) query.orWhere('monitorId', scope.monitorId)
      })
      .orderBy('id', 'asc')
  }

  async findAll(): Promise<AlertRule[]> {
    return AlertRule.query().orderBy('id', 'asc')
  }

  async count(): Promise<number> {
    const result = await AlertRule.query().count('* as total').first()
    return Number(result?.$extras?.total ?? 0)
  }
}
