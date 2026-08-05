import type Monitor from '#models/monitor'
import type { CheckResult } from '#modules/monitoring/contracts/check_result'
import { ALERT_FIELDS } from '../alert_fields.js'
import type { AlertDataset } from '../contracts/alert_evaluation.js'

/**
 * Nomes das métricas produzidas pelos checkers -> chaves usadas nas condições
 * das regras de alerta. Mantém o vocabulário da UI alinhado ao avaliador.
 */
const METRIC_FIELD_MAP: Record<string, string> = {
  latency: ALERT_FIELDS.latencyMs,
  response_time: ALERT_FIELDS.latencyMs,
  packet_loss: ALERT_FIELDS.packetLoss,
  status_code: ALERT_FIELDS.statusCode,
  connect_time: ALERT_FIELDS.connectTimeMs,
  resolution_time: ALERT_FIELDS.resolutionTimeMs,
  if_oper_status: ALERT_FIELDS.ifOperStatus,
  if_speed: ALERT_FIELDS.ifSpeed,
  snmp_uptime: ALERT_FIELDS.snmpUptime,
}

/** Traduz o CheckResult de um monitor para o vocabulário avaliado pelas regras. */
export function buildMonitorResultDataset(monitor: Monitor, result: CheckResult): AlertDataset {
  const dataset: AlertDataset = {
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
    if (
      field === ALERT_FIELDS.latencyMs &&
      dataset.latencyMs !== null &&
      metric.name === 'response_time'
    ) {
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
