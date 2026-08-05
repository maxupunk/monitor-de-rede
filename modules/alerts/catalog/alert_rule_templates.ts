import type AlertRule from '#models/alert_rule'
import {
  ALERT_FIELDS,
  INTERFACE_SPEED_TRANSITION,
  INTERFACE_STATUS_TRANSITION,
} from '../alert_fields.js'
import type { AlertRuleCondition } from '../rule_evaluator.js'

/**
 * Catálogo de regras pré-configuradas.
 *
 * É a única fonte de verdade das políticas de alerta que antes viviam
 * espalhadas no código (ex.: downgrade de negociação de interface). Cada
 * template é apenas *dado*: quem aplica, avalia e dispara não precisa ser
 * alterado para nascer uma política nova — basta acrescentar um item aqui.
 */

export type AlertRuleCategory =
  'disponibilidade' | 'desempenho' | 'servicos' | 'interfaces' | 'equipamento'

export interface AlertRuleTemplate {
  /** Chave de idempotência: uma regra por template */
  key: string
  name: string
  description: string
  category: AlertRuleCategory
  type: AlertRule['type']
  condition: AlertRuleCondition
  severity: AlertRule['severity']
  durationSeconds: number
  /** Faz parte do conjunto básico provisionado por padrão */
  recommended: boolean
}

export const ALERT_RULE_CATEGORY_LABELS: Record<AlertRuleCategory, string> = {
  disponibilidade: 'Disponibilidade',
  desempenho: 'Desempenho',
  servicos: 'Serviços e aplicações',
  interfaces: 'Interfaces de rede (SNMP)',
  equipamento: 'Equipamento (SNMP)',
}

export const ALERT_RULE_TEMPLATES: readonly AlertRuleTemplate[] = [
  {
    key: 'device_offline',
    name: 'Dispositivo sem resposta',
    description:
      'Dispara assim que uma verificação retorna "sem resposta". É a regra base de indisponibilidade.',
    category: 'disponibilidade',
    type: 'device_offline',
    condition: { field: ALERT_FIELDS.status, operator: 'eq', value: 'down' },
    severity: 'critical',
    durationSeconds: 0,
    recommended: true,
  },
  {
    key: 'packet_loss_high',
    name: 'Perda de pacotes acima de 10%',
    description:
      'Link instável: parte dos pacotes ICMP não volta. Só dispara se a perda persistir por 5 minutos.',
    category: 'disponibilidade',
    type: 'custom',
    condition: { field: ALERT_FIELDS.packetLoss, operator: 'gt', value: 10 },
    severity: 'warning',
    durationSeconds: 300,
    recommended: true,
  },
  {
    key: 'latency_high',
    name: 'Latência acima de 200 ms',
    description:
      'Tempo de resposta degradado de forma sustentada (5 minutos), evitando alarme por oscilação momentânea.',
    category: 'desempenho',
    type: 'latency_high',
    condition: { field: ALERT_FIELDS.latencyMs, operator: 'gt', value: 200 },
    severity: 'warning',
    durationSeconds: 300,
    recommended: true,
  },
  {
    key: 'latency_critical',
    name: 'Latência acima de 500 ms',
    description:
      'Degradação severa de latência: acima deste patamar aplicações interativas travam.',
    category: 'desempenho',
    type: 'latency_high',
    condition: { field: ALERT_FIELDS.latencyMs, operator: 'gt', value: 500 },
    severity: 'critical',
    durationSeconds: 300,
    recommended: false,
  },
  {
    key: 'check_duration_high',
    name: 'Checagem demorando mais de 5 segundos',
    description: 'Verificação lenta como um todo — útil para monitores HTTP de páginas pesadas.',
    category: 'desempenho',
    type: 'custom',
    condition: { field: ALERT_FIELDS.durationMs, operator: 'gt', value: 5000 },
    severity: 'info',
    durationSeconds: 300,
    recommended: false,
  },
  {
    key: 'http_error_response',
    name: 'Serviço HTTP respondendo com erro (4xx/5xx)',
    description: 'O site/serviço responde, mas devolve código de erro em vez de conteúdo válido.',
    category: 'servicos',
    type: 'http_failure',
    condition: { field: ALERT_FIELDS.statusCode, operator: 'gte', value: 400 },
    severity: 'critical',
    durationSeconds: 0,
    recommended: true,
  },
  {
    key: 'tcp_connect_slow',
    name: 'Conexão TCP acima de 1 segundo',
    description: 'A porta monitorada aceita conexão, mas o handshake está lento.',
    category: 'servicos',
    type: 'tcp_failure',
    condition: { field: ALERT_FIELDS.connectTimeMs, operator: 'gt', value: 1000 },
    severity: 'warning',
    durationSeconds: 300,
    recommended: false,
  },
  {
    key: 'dns_resolution_slow',
    name: 'Resolução DNS acima de 800 ms',
    description: 'O servidor DNS responde, porém devagar o bastante para atrasar toda a navegação.',
    category: 'servicos',
    type: 'custom',
    condition: { field: ALERT_FIELDS.resolutionTimeMs, operator: 'gt', value: 800 },
    severity: 'warning',
    durationSeconds: 300,
    recommended: false,
  },
  {
    key: 'interface_link_down',
    name: 'Interface de rede caiu (UP ➔ DOWN)',
    description:
      'Queda de link detectada na coleta SNMP das interfaces administrativamente habilitadas.',
    category: 'interfaces',
    type: 'custom',
    condition: {
      field: ALERT_FIELDS.interfaceStatusTransition,
      operator: 'eq',
      value: INTERFACE_STATUS_TRANSITION.wentDown,
    },
    severity: 'warning',
    durationSeconds: 0,
    recommended: true,
  },
  {
    key: 'interface_speed_downgrade',
    name: 'Downgrade na negociação da interface',
    description:
      'A interface renegociou para uma velocidade menor (ex.: 1 Gbps ➔ 100 Mbps) — sintoma clássico de cabo ou porta com defeito.',
    category: 'interfaces',
    type: 'custom',
    condition: {
      field: ALERT_FIELDS.interfaceSpeedTransition,
      operator: 'eq',
      value: INTERFACE_SPEED_TRANSITION.downgrade,
    },
    severity: 'warning',
    durationSeconds: 0,
    recommended: true,
  },
  {
    key: 'interface_speed_upgrade',
    name: 'Interface renegociou velocidade para cima',
    description:
      'Registro informativo de renegociação para uma velocidade maior (ex.: 100 Mbps ➔ 1 Gbps).',
    category: 'interfaces',
    type: 'custom',
    condition: {
      field: ALERT_FIELDS.interfaceSpeedTransition,
      operator: 'eq',
      value: INTERFACE_SPEED_TRANSITION.upgrade,
    },
    severity: 'info',
    durationSeconds: 0,
    recommended: false,
  },
  {
    key: 'interface_link_recovered',
    name: 'Interface voltou a operar (DOWN ➔ UP)',
    description: 'Registro informativo do retorno do link após uma queda.',
    category: 'interfaces',
    type: 'custom',
    condition: {
      field: ALERT_FIELDS.interfaceStatusTransition,
      operator: 'eq',
      value: INTERFACE_STATUS_TRANSITION.cameBack,
    },
    severity: 'info',
    durationSeconds: 0,
    recommended: false,
  },
  {
    key: 'interface_below_gigabit',
    name: 'Interface negociada abaixo de 1 Gbps',
    description:
      'Vigia links que deveriam operar em gigabit e estão negociando abaixo disso de forma contínua.',
    category: 'interfaces',
    type: 'custom',
    condition: { field: ALERT_FIELDS.interfaceSpeedBps, operator: 'lt', value: 1_000_000_000 },
    severity: 'info',
    durationSeconds: 0,
    recommended: false,
  },
  {
    key: 'snmp_interface_oper_down',
    name: 'Interface monitorada via SNMP inativa',
    description:
      'Para monitores do tipo SNMP que leem ifOperStatus diretamente: 2 significa interface inativa.',
    category: 'equipamento',
    type: 'custom',
    condition: { field: ALERT_FIELDS.ifOperStatus, operator: 'eq', value: '2' },
    severity: 'warning',
    durationSeconds: 0,
    recommended: false,
  },
  {
    key: 'snmp_device_restarted',
    name: 'Equipamento reiniciado recentemente',
    description:
      'Uptime SNMP abaixo de 10 minutos indica que o equipamento reiniciou (queda de energia, travamento ou reboot).',
    category: 'equipamento',
    type: 'custom',
    // sysUpTime é reportado em centésimos de segundo: 60.000 = 10 minutos
    condition: { field: ALERT_FIELDS.snmpUptime, operator: 'lt', value: 60_000 },
    severity: 'warning',
    durationSeconds: 0,
    recommended: false,
  },
] as const

export function findTemplate(key: string): AlertRuleTemplate | undefined {
  return ALERT_RULE_TEMPLATES.find((template) => template.key === key)
}

export function recommendedTemplates(): AlertRuleTemplate[] {
  return ALERT_RULE_TEMPLATES.filter((template) => template.recommended)
}
