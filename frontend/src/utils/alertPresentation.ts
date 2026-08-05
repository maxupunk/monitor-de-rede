/**
 * Tradução das métricas/condições técnicas das regras de alerta para rótulos
 * legíveis em português. O `field` de cada métrica corresponde exatamente à
 * chave produzida pelo AlertManager no backend (modules/alerts/alert_manager.ts).
 */

export type AlertOperator = 'gt' | 'gte' | 'lt' | 'lte' | 'eq' | 'neq' | 'contains'

export interface AlertMetricOption {
  /** Chave avaliada pelo RuleEvaluator */
  field: string
  /** Rótulo exibido no select */
  title: string
  /** Explicação curta mostrada abaixo do campo */
  hint: string
  /** Define quais operadores fazem sentido e como o valor é digitado */
  kind: 'number' | 'enum'
  /** Sufixo exibido no campo de valor (ms, %, ...) */
  unit?: string
  /** Opções fixas quando `kind === 'enum'` */
  options?: Array<{ value: string; title: string }>
  /** Sugestões aplicadas ao escolher a métrica */
  defaultOperator: AlertOperator
  defaultValue: number | string
}

export const ALERT_METRICS: AlertMetricOption[] = [
  {
    field: 'latencyMs',
    title: 'Latência de resposta (ms)',
    hint: 'Tempo que o equipamento levou para responder ao teste (ping, HTTP ou TCP).',
    kind: 'number',
    unit: 'ms',
    defaultOperator: 'gt',
    defaultValue: 150,
  },
  {
    field: 'packetLoss',
    title: 'Perda de pacotes (%)',
    hint: 'Percentual de pacotes ICMP perdidos. 100% significa host inacessível.',
    kind: 'number',
    unit: '%',
    defaultOperator: 'gt',
    defaultValue: 10,
  },
  {
    field: 'status',
    title: 'Resultado da verificação',
    hint: 'Situação apurada na última checagem do monitor.',
    kind: 'enum',
    options: [
      { value: 'up', title: 'Respondendo (online)' },
      { value: 'down', title: 'Sem resposta (offline)' },
      { value: 'warning', title: 'Instável (com falhas parciais)' },
      { value: 'unknown', title: 'Indeterminado' },
    ],
    defaultOperator: 'eq',
    defaultValue: 'down',
  },
  {
    field: 'statusCode',
    title: 'Código de resposta HTTP',
    hint: 'Código devolvido pelo site/serviço monitorado. Ex.: 200 = OK, 500 = erro.',
    kind: 'number',
    defaultOperator: 'gte',
    defaultValue: 400,
  },
  {
    field: 'durationMs',
    title: 'Tempo total da checagem (ms)',
    hint: 'Duração completa da verificação, do início ao fim.',
    kind: 'number',
    unit: 'ms',
    defaultOperator: 'gt',
    defaultValue: 3000,
  },
  {
    field: 'connectTimeMs',
    title: 'Tempo de conexão TCP (ms)',
    hint: 'Tempo até abrir a conexão na porta monitorada.',
    kind: 'number',
    unit: 'ms',
    defaultOperator: 'gt',
    defaultValue: 1000,
  },
  {
    field: 'resolutionTimeMs',
    title: 'Tempo de resolução DNS (ms)',
    hint: 'Tempo que o servidor DNS levou para resolver o nome consultado.',
    kind: 'number',
    unit: 'ms',
    defaultOperator: 'gt',
    defaultValue: 800,
  },
  {
    field: 'ifOperStatus',
    title: 'Status operacional da interface (SNMP)',
    hint: 'Leitura SNMP ifOperStatus: 1 = ativa (up), 2 = inativa (down).',
    kind: 'enum',
    options: [
      { value: '1', title: 'Ativa (up)' },
      { value: '2', title: 'Inativa (down)' },
    ],
    defaultOperator: 'eq',
    defaultValue: '2',
  },
  {
    field: 'ifSpeed',
    title: 'Velocidade negociada da interface (bps)',
    hint: 'Velocidade do link via SNMP. Ex.: 100000000 = 100 Mbps.',
    kind: 'number',
    unit: 'bps',
    defaultOperator: 'lt',
    defaultValue: 100000000,
  },
  {
    field: 'snmpUptime',
    title: 'Tempo ligado do equipamento (SNMP)',
    hint: 'Uptime em centésimos de segundo. Valor baixo indica reinício recente.',
    kind: 'number',
    defaultOperator: 'lt',
    defaultValue: 6000,
  },
]

interface OperatorOption {
  value: AlertOperator
  title: string
  /** Texto usado na frase-resumo da regra */
  phrase: string
  kinds: Array<AlertMetricOption['kind']>
}

export const ALERT_OPERATORS: OperatorOption[] = [
  { value: 'gt', title: 'For maior que', phrase: 'for maior que', kinds: ['number'] },
  {
    value: 'gte',
    title: 'For maior ou igual a',
    phrase: 'for maior ou igual a',
    kinds: ['number'],
  },
  { value: 'lt', title: 'For menor que', phrase: 'for menor que', kinds: ['number'] },
  {
    value: 'lte',
    title: 'For menor ou igual a',
    phrase: 'for menor ou igual a',
    kinds: ['number'],
  },
  { value: 'eq', title: 'For igual a', phrase: 'for igual a', kinds: ['number', 'enum'] },
  {
    value: 'neq',
    title: 'For diferente de',
    phrase: 'for diferente de',
    kinds: ['number', 'enum'],
  },
  { value: 'contains', title: 'Contiver o texto', phrase: 'contiver o texto', kinds: ['enum'] },
]

export const ALERT_SEVERITIES = [
  { value: 'info', title: 'Informativo — apenas registra', color: 'info' },
  { value: 'warning', title: 'Atenção — requer acompanhamento', color: 'warning' },
  { value: 'critical', title: 'Crítico — exige ação imediata', color: 'error' },
] as const

/** Janelas de tolerância antes de disparar o alerta */
export const ALERT_DURATIONS = [
  { value: 0, title: 'Disparar na primeira ocorrência' },
  { value: 60, title: 'Somente se persistir por 1 minuto' },
  { value: 300, title: 'Somente se persistir por 5 minutos' },
  { value: 900, title: 'Somente se persistir por 15 minutos' },
  { value: 1800, title: 'Somente se persistir por 30 minutos' },
]

export function findMetric(field?: string): AlertMetricOption | undefined {
  return ALERT_METRICS.find((m) => m.field === field)
}

export function operatorsForMetric(field?: string): OperatorOption[] {
  const metric = findMetric(field)
  if (!metric) return ALERT_OPERATORS
  return ALERT_OPERATORS.filter((op) => op.kinds.includes(metric.kind))
}

export function metricLabel(field?: string): string {
  return findMetric(field)?.title ?? field ?? '—'
}

export function operatorLabel(operator?: string): string {
  return ALERT_OPERATORS.find((op) => op.value === operator)?.title ?? operator ?? '—'
}

export function severityLabel(severity?: string): string {
  switch (severity) {
    case 'critical':
      return 'Crítico'
    case 'error':
      return 'Erro'
    case 'warning':
      return 'Atenção'
    case 'info':
      return 'Informativo'
    default:
      return (severity || 'info').toUpperCase()
  }
}

export function severityColor(severity?: string): string {
  switch (severity) {
    case 'critical':
      return 'error'
    case 'error':
      return 'deep-orange'
    case 'warning':
      return 'warning'
    default:
      return 'info'
  }
}

export function statusLabel(status?: string): string {
  switch (status) {
    case 'active':
      return 'Ativo'
    case 'acknowledged':
      return 'Reconhecido'
    case 'silenced':
      return 'Silenciado'
    case 'resolved':
      return 'Resolvido'
    default:
      return (status || 'active').toUpperCase()
  }
}

/** Formata o valor levando em conta rótulos de enum e unidade da métrica */
export function formatConditionValue(field?: string, value?: unknown): string {
  const metric = findMetric(field)
  if (!metric) return String(value ?? '—')

  if (metric.kind === 'enum') {
    const match = metric.options?.find((opt) => opt.value === String(value))
    return match ? match.title : String(value ?? '—')
  }

  return metric.unit ? `${value} ${metric.unit}` : String(value ?? '—')
}

export interface RuleCondition {
  field?: string
  operator?: string
  value?: unknown
}

/** Frase completa: "Latência de resposta (ms) for maior que 150 ms" */
export function describeCondition(condition?: RuleCondition | null): string {
  if (!condition?.field || !condition?.operator) return 'Condição não configurada'
  const phrase = ALERT_OPERATORS.find((op) => op.value === condition.operator)?.phrase
  return `${metricLabel(condition.field)} ${phrase ?? condition.operator} ${formatConditionValue(condition.field, condition.value)}`
}

export function describeRule(condition?: RuleCondition | null, durationSeconds = 0): string {
  const base = `Alertar quando ${describeCondition(condition)}`
  if (!durationSeconds) return `${base}.`
  const window = ALERT_DURATIONS.find((d) => d.value === durationSeconds)
  return `${base}, ${(window?.title ?? `por ${durationSeconds}s`).toLowerCase()}.`
}
