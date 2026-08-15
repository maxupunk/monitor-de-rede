/**
 * Tradução das métricas/condições técnicas das regras de alerta para rótulos
 * legíveis em português. O `field` de cada métrica corresponde exatamente à
 * chave produzida pelo AlertManager no backend (modules/alerts/alert_manager.ts).
 */

import { formatBps } from './formatters'
import { getStatusColor } from './monitorPresentation'

export type AlertOperator = 'gt' | 'gte' | 'lt' | 'lte' | 'eq' | 'neq' | 'contains'

export interface AlertMetricOption {
  /** Chave avaliada pelo RuleEvaluator */
  field: string
  /** Rótulo exibido no select */
  title: string
  /** Explicação curta mostrada abaixo do campo */
  hint: string
  /** Define quais operadores fazem sentido e como o valor é digitado */
  kind: 'number' | 'enum' | 'text'
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
  {
    field: 'inBps',
    title: 'Tráfego de entrada da interface (bps)',
    hint: 'Taxa de download/recebimento de dados medida na interface.',
    kind: 'number',
    unit: 'bps',
    defaultOperator: 'gt',
    defaultValue: 100000000,
  },
  {
    field: 'outBps',
    title: 'Tráfego de saída da interface (bps)',
    hint: 'Taxa de upload/envio de dados medida na interface.',
    kind: 'number',
    unit: 'bps',
    defaultOperator: 'gt',
    defaultValue: 100000000,
  },
  {
    field: 'interfaceStatusTransition',
    title: 'Mudança de estado da interface (SNMP)',
    hint: 'Comparação entre a coleta anterior e a atual das interfaces do equipamento.',
    kind: 'enum',
    options: [
      { value: 'up_to_down', title: 'A interface caiu (UP ➔ DOWN)' },
      { value: 'down_to_up', title: 'A interface voltou (DOWN ➔ UP)' },
    ],
    defaultOperator: 'eq',
    defaultValue: 'up_to_down',
  },
  {
    field: 'interfaceSpeedTransition',
    title: 'Renegociação de velocidade da interface',
    hint: 'Downgrade indica queda na velocidade negociada (ex.: 1 Gbps ➔ 100 Mbps).',
    kind: 'enum',
    options: [
      { value: 'downgrade', title: 'Downgrade (negociou para menos)' },
      { value: 'upgrade', title: 'Upgrade (negociou para mais)' },
    ],
    defaultOperator: 'eq',
    defaultValue: 'downgrade',
  },
  {
    field: 'interfaceSpeedBps',
    title: 'Velocidade atual da interface (bps)',
    hint: 'Velocidade negociada na última coleta. Ex.: 1000000000 = 1 Gbps.',
    kind: 'number',
    unit: 'bps',
    defaultOperator: 'lt',
    defaultValue: 1000000000,
  },
  {
    field: 'interfaceOperStatus',
    title: 'Estado atual da interface (SNMP)',
    hint: 'Situação da interface na última coleta, independentemente de ter mudado.',
    kind: 'enum',
    options: [
      { value: 'up', title: 'Operando (up)' },
      { value: 'down', title: 'Inativa (down)' },
    ],
    defaultOperator: 'eq',
    defaultValue: 'down',
  },
  {
    field: 'interfaceName',
    title: 'Nome da interface',
    hint: 'Restringe a regra a interfaces específicas. Ex.: contiver "uplink" ou "wan".',
    kind: 'text',
    defaultOperator: 'contains',
    defaultValue: '',
  },
  {
    field: 'vpnStatusTransition',
    title: 'Mudança de estado do túnel VPN',
    hint: 'Comparação entre o ciclo anterior e o atual da telemetria do WireGuard.',
    kind: 'enum',
    options: [
      { value: 'connected_to_disconnected', title: 'O túnel caiu' },
      { value: 'connected_to_unstable', title: 'O túnel ficou instável' },
      { value: 'reconnected', title: 'O túnel voltou' },
    ],
    defaultOperator: 'eq',
    defaultValue: 'connected_to_disconnected',
  },
  {
    field: 'vpnPeerStatus',
    title: 'Estado atual do túnel VPN',
    hint: 'Situação do túnel na última sincronização, independentemente de ter mudado.',
    kind: 'enum',
    options: [
      { value: 'connected', title: 'Conectado' },
      { value: 'unstable', title: 'Instável' },
      { value: 'disconnected', title: 'Desconectado' },
      { value: 'awaiting', title: 'Aguardando primeira conexão' },
    ],
    defaultOperator: 'eq',
    defaultValue: 'disconnected',
  },
  {
    field: 'vpnSecondsSinceActivity',
    title: 'Tempo sem sinal do túnel VPN (s)',
    hint: 'Segundos desde o último keepalive ou handshake recebido do peer.',
    kind: 'number',
    unit: 's',
    defaultOperator: 'gt',
    defaultValue: 300,
  },
  {
    field: 'vpnPeerName',
    title: 'Nome do equipamento na VPN',
    hint: 'Restringe a regra a túneis específicos. Ex.: contiver "filial".',
    kind: 'text',
    defaultOperator: 'contains',
    defaultValue: '',
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
  { value: 'eq', title: 'For igual a', phrase: 'for igual a', kinds: ['number', 'enum', 'text'] },
  {
    value: 'neq',
    title: 'For diferente de',
    phrase: 'for diferente de',
    kinds: ['number', 'enum', 'text'],
  },
  {
    value: 'contains',
    title: 'Contiver o texto',
    phrase: 'contiver o texto',
    kinds: ['enum', 'text'],
  },
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

/**
 * Janelas de estabilização antes de resolver o alerta. Enquanto a janela não
 * vence, o evento fica em `recovering`; cada recaída reinicia a contagem.
 * `phrase` é a cláusula usada na frase-resumo de `describeRule`.
 */
export const RECOVERY_WINDOWS = [
  {
    value: 0,
    title: 'Sem estabilização — resolve na 1ª checagem ok',
    phrase: 'resolve na primeira checagem ok',
  },
  {
    value: 120,
    title: 'Resolver após 2 min sem recaída',
    phrase: 'resolve após 2 min sem recaída',
  },
  {
    value: 300,
    title: 'Resolver após 5 min sem recaída',
    phrase: 'resolve após 5 min sem recaída',
  },
  {
    value: 900,
    title: 'Resolver após 15 min sem recaída',
    phrase: 'resolve após 15 min sem recaída',
  },
  {
    value: 1800,
    title: 'Resolver após 30 min sem recaída',
    phrase: 'resolve após 30 min sem recaída',
  },
]

/**
 * Limiares de oscilação: quantas recaídas dentro da janela declaram o alvo
 * "cronicamente instável" (estado `flapping`). Zero desliga a detecção.
 *
 * A detecção acontece sobre o episódio, que só sobrevive à oscilação quando há
 * janela de estabilização — por isso o formulário avisa quando o limiar está
 * ligado com `recoveryWindowSeconds` em zero.
 */
export const FLAP_THRESHOLDS = [
  {
    value: 0,
    title: 'Não detectar oscilação',
    phrase: 'sem detecção de oscilação',
  },
  { value: 3, title: 'Após 3 recaídas na janela', phrase: 'marca como oscilando após 3 recaídas' },
  { value: 5, title: 'Após 5 recaídas na janela', phrase: 'marca como oscilando após 5 recaídas' },
  {
    value: 10,
    title: 'Após 10 recaídas na janela',
    phrase: 'marca como oscilando após 10 recaídas',
  },
]

/** Largura da janela deslizante em que as recaídas são contadas */
export const FLAP_WINDOWS = [
  { value: 300, title: 'Contar recaídas dos últimos 5 minutos', short: '5 min' },
  { value: 900, title: 'Contar recaídas dos últimos 15 minutos', short: '15 min' },
  { value: 3600, title: 'Contar recaídas da última hora', short: '1 hora' },
  { value: 21600, title: 'Contar recaídas das últimas 6 horas', short: '6 horas' },
]

/**
 * Intervalo mínimo entre notificações de problema do mesmo alvo, **mesmo quando
 * o alerta fecha e um novo abre**. É a lacuna que a estabilização não cobre: ela
 * segura a oscilação dentro do episódio, mas nada impedia um episódio de fechar
 * e outro abrir três minutos depois, com o par 🚨+✅ inteiro de novo.
 *
 * O ✅ acompanha o 🚨: quando o disparo é engolido pelo cooldown, a resolução
 * dele também é — avisar que voltou algo que ninguém soube que caiu é ruído.
 */
export const NOTIFICATION_COOLDOWNS = [
  {
    value: 0,
    title: 'Sem intervalo — notificar toda vez que reabrir',
    phrase: 'notifica toda reabertura',
  },
  {
    value: 300,
    title: 'No máximo uma notificação a cada 5 min',
    phrase: 'no máximo 1 aviso/5 min',
  },
  {
    value: 900,
    title: 'No máximo uma notificação a cada 15 min',
    phrase: 'no máximo 1 aviso/15 min',
  },
  { value: 3600, title: 'No máximo uma notificação por hora', phrase: 'no máximo 1 aviso/hora' },
  {
    value: 21600,
    title: 'No máximo uma notificação a cada 6 horas',
    phrase: 'no máximo 1 aviso/6 h',
  },
]

/**
 * Classificação do problema que abriu o episódio, preenchida pelo backend em
 * `AlertEvent.data.problemKind`. A união com `string & {}` mantém o
 * autocomplete dos valores conhecidos sem rejeitar valores futuros.
 */
export type AlertProblemKind =
  | 'down'
  | 'packet_loss'
  | 'latency'
  | 'dns_failure'
  | 'interface_flap'
  | 'vpn_instability'
  | (string & {})

const PROBLEM_KIND_LABELS: Record<string, string> = {
  down: 'Indisponível',
  packet_loss: 'Perda de pacotes',
  latency: 'Latência alta',
  dns_failure: 'Falha de DNS',
  interface_flap: 'Interface oscilando',
  vpn_instability: 'Instabilidade VPN',
}

/**
 * Rótulo pt-BR do tipo de problema. Ausente ou desconhecido retorna `null`:
 * quem consome simplesmente não renderiza o chip.
 */
export function problemKindLabel(kind?: string | null): string | null {
  if (!kind) return null
  return PROBLEM_KIND_LABELS[kind] ?? null
}

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
    case 'recovering':
      return 'Estabilizando'
    case 'flapping':
      return 'Oscilando'
    case 'resolved':
      return 'Resolvido'
    default:
      return (status || 'active').toUpperCase()
  }
}

/**
 * Cor do chip de status do alerta, resolvida pelo StatusTone central. Só os
 * estados intermediários ganham cor própria: nos demais, a severidade continua
 * sendo a cor dominante e o chip fica no outlined neutro.
 */
export function statusColor(status?: string): string | undefined {
  return status === 'recovering' || status === 'flapping' ? getStatusColor(status) : undefined
}

/** Formata o valor levando em conta rótulos de enum e unidade da métrica */
export function formatConditionValue(field?: string, value?: unknown): string {
  const metric = findMetric(field)
  if (!metric) return String(value ?? '—')

  if (metric.kind === 'enum') {
    const match = metric.options?.find((opt) => opt.value === String(value))
    return match ? match.title : String(value ?? '—')
  }

  if (metric.unit === 'bps') return formatBps(Number(value))

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

export interface RuleNotificationOptions {
  /** Intervalo mínimo entre notificações do mesmo alvo (0 = sem intervalo) */
  notificationCooldownSeconds?: number
  /** Suprimir quando o equipamento-pai já está em alerta */
  inhibitWhenParentDown?: boolean
}

export function describeRule(
  condition?: RuleCondition | null,
  durationSeconds = 0,
  recoveryWindowSeconds = 0,
  flapThreshold = 0,
  flapWindowSeconds = 900,
  notification: RuleNotificationOptions = {}
): string {
  const base = `Alertar quando ${describeCondition(condition)}`
  const duration = durationSeconds
    ? `, ${(ALERT_DURATIONS.find((d) => d.value === durationSeconds)?.title ?? `por ${durationSeconds}s`).toLowerCase()}`
    : ''
  const recovery =
    RECOVERY_WINDOWS.find((w) => w.value === recoveryWindowSeconds)?.phrase ??
    `resolve após ${recoveryWindowSeconds}s sem recaída`
  // A cláusula de oscilação só aparece quando há detecção configurada: dizer
  // "sem detecção de oscilação" em toda regra seria ruído.
  const flap = flapThreshold
    ? `; marca como oscilando após ${flapThreshold} recaídas em ${flapWindowLabel(flapWindowSeconds)}`
    : ''
  // Idem para as cláusulas de higiene de notificação: só entram na frase quando
  // mudam o comportamento padrão.
  const cooldown = notification.notificationCooldownSeconds
    ? `; ${
        NOTIFICATION_COOLDOWNS.find((c) => c.value === notification.notificationCooldownSeconds)
          ?.phrase ?? `no máximo 1 aviso/${notification.notificationCooldownSeconds}s`
      }`
    : ''
  const inhibition = notification.inhibitWhenParentDown
    ? '; silencia quando o equipamento-pai já está em alerta'
    : ''
  return `${base}${duration}; ${recovery}${flap}${cooldown}${inhibition}.`
}

/** "15 min" / "1 hora" — a janela de flap em texto curto */
export function flapWindowLabel(seconds?: number): string {
  const known = FLAP_WINDOWS.find((w) => w.value === seconds)
  if (known) return known.short
  return `${Math.round((seconds ?? 0) / 60)} min`
}

/**
 * A detecção de flapping é medida sobre o episódio, e o episódio só sobrevive à
 * oscilação quando existe janela de estabilização. Limiar ligado com janela
 * zerada nunca dispara — o formulário precisa dizer isso antes de salvar.
 */
export function flapNeedsRecoveryWindow(
  flapThreshold?: number,
  recoveryWindowSeconds?: number
): boolean {
  return (flapThreshold ?? 0) > 0 && (recoveryWindowSeconds ?? 0) === 0
}
