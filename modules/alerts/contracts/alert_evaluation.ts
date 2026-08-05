/**
 * Contratos da avaliação de alertas.
 *
 * Quem observa a rede (monitor, coleta SNMP, ...) só produz *fatos*; quem
 * decide se aquilo vira alerta — e com qual severidade — são as regras
 * cadastradas. Este contrato é a fronteira entre os dois lados: nenhum produtor
 * de fatos precisa conhecer AlertEvent, notificação ou severidade.
 */

/** Fatos observados no vocabulário avaliado pelas regras (`condition.field`). */
export type AlertDataset = Record<string, unknown>

/** Delimita quais regras se aplicam ao fato observado (null = regra global). */
export interface AlertEvaluationScope {
  siteId: number | null
  deviceId: number | null
  monitorId: number | null
}

export interface AlertEvaluationContext {
  scope: AlertEvaluationScope

  /**
   * Identidade do alvo avaliado (`monitor:12`, `interface:34`). Deduplica os
   * eventos ativos e delimita a normalização automática.
   */
  scopeKey: string

  /** Rótulo do alvo exibido no título do alerta. */
  targetLabel: string

  dataset: AlertDataset

  /** Descrição legível do fato; cai para o nome da regra quando ausente. */
  message?: string | null

  /** Conteúdo extra persistido em `alert_events.data`. */
  data?: Record<string, unknown>

  /**
   * `true` quando o alvo voltou ao normal. Se nenhuma regra disparar nesta
   * avaliação, os alertas abertos do escopo são resolvidos.
   */
  recovered?: boolean
}

/** Chaves de escopo — centralizadas para produtor e consumidor não divergirem. */
export const AlertScopeKey = {
  monitor: (monitorId: number): string => `monitor:${monitorId}`,
  interface: (interfaceId: number): string => `interface:${interfaceId}`,
} as const
