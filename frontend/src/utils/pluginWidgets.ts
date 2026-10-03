/**
 * Componentes de campo dos formulários de plugin (`widget` no esquema) e a
 * regra que mostra um campo conforme outro (`visibleWhen`).
 *
 * Espelha `backend/src/services/plugins/widgets.rs`: o backend valida de
 * verdade, aqui é o retorno imediato na tela (a mesma mensagem, para o
 * operador não ver duas versões do mesmo erro).
 */

export type Widget =
  'ip' | 'cidr' | 'mac' | 'port' | 'hostname' | 'url' | 'textarea' | 'slider' | 'tags'

const WIDGETS: readonly Widget[] = [
  'ip',
  'cidr',
  'mac',
  'port',
  'hostname',
  'url',
  'textarea',
  'slider',
  'tags',
]

export function widgetOf(value: unknown): Widget | undefined {
  return WIDGETS.find((widget) => widget === value)
}

interface WidgetSpec {
  /** Exemplo no campo vazio. */
  placeholder?: string
  /** Ícone dentro do campo. */
  icon?: string
  /** Confere o valor; devolve a mensagem quando não serve. */
  check?: (value: string) => string | null
}

const IPV4 = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/

function isIpv6(text: string): boolean {
  if (!/^[0-9a-fA-F:.]+$/.test(text) || !text.includes(':')) return false
  const doubles = text.split('::').length - 1
  if (doubles > 1) return false
  const groups = text.split(':').filter((group) => group !== '')
  return (
    groups.length <= 8 &&
    groups.every((group) => IPV4.test(group) || /^[0-9a-fA-F]{1,4}$/.test(group))
  )
}

export function isIp(text: string): boolean {
  return IPV4.test(text) || isIpv6(text)
}

function isCidr(text: string): boolean {
  const [address, prefix, extra] = text.split('/')
  if (extra !== undefined || prefix === undefined || !/^\d{1,3}$/.test(prefix)) return false
  const bits = Number(prefix)
  if (IPV4.test(address ?? '')) return bits <= 32
  return isIpv6(address ?? '') && bits <= 128
}

function isMac(text: string): boolean {
  return (
    /^([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}$/.test(text) ||
    /^([0-9a-fA-F]{2}-){5}[0-9a-fA-F]{2}$/.test(text)
  )
}

function isHostname(text: string): boolean {
  return (
    text.length > 0 &&
    text.length <= 253 &&
    text.split('.').every((label) => /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/.test(label))
  )
}

function isUrl(text: string): boolean {
  try {
    const url = new URL(text)
    return (url.protocol === 'http:' || url.protocol === 'https:') && url.hostname !== ''
  } catch {
    return false
  }
}

const SPECS: Record<Widget, WidgetSpec> = {
  ip: {
    placeholder: '192.168.1.1',
    icon: 'mdi-ip-network-outline',
    check: (text) => (isIp(text) ? null : 'Precisa ser um endereço IP (ex.: 192.168.1.1)'),
  },
  cidr: {
    placeholder: '192.168.1.0/24',
    icon: 'mdi-lan',
    check: (text) => (isCidr(text) ? null : 'Precisa ser uma rede no formato 192.168.1.0/24'),
  },
  mac: {
    placeholder: 'AA:BB:CC:DD:EE:FF',
    icon: 'mdi-ethernet',
    check: (text) => (isMac(text) ? null : 'Precisa ser um MAC (ex.: AA:BB:CC:DD:EE:FF)'),
  },
  port: {
    placeholder: '443',
    icon: 'mdi-numeric',
    check: (text) => {
      const port = Number(text)
      return Number.isInteger(port) && port >= 1 && port <= 65535
        ? null
        : 'Precisa ser uma porta de 1 a 65535'
    },
  },
  hostname: {
    placeholder: 'roteador.lan',
    icon: 'mdi-dns-outline',
    check: (text) =>
      isHostname(text) ? null : 'Precisa ser um nome de host (letras, números, hífen e ponto)',
  },
  url: {
    placeholder: 'https://exemplo.com',
    icon: 'mdi-link-variant',
    check: (text) => (isUrl(text) ? null : 'Precisa ser um endereço http:// ou https://'),
  },
  textarea: {},
  slider: {},
  tags: {},
}

export function widgetPlaceholder(widget: Widget | undefined): string | undefined {
  return widget ? SPECS[widget].placeholder : undefined
}

export function widgetIcon(widget: Widget | undefined): string | undefined {
  return widget ? SPECS[widget].icon : undefined
}

/** Regra de tela do componente (vazio passa: o obrigatório é outra regra). */
export function widgetRule(widget: Widget | undefined): ((value: unknown) => true | string) | null {
  const check = widget ? SPECS[widget].check : undefined
  if (!check) return null
  return (value) => {
    if (value === '' || value === null || value === undefined) return true
    return check(String(value)) ?? true
  }
}

/** A regra `visibleWhen` de um campo. */
export interface VisibleWhen {
  field: string
  equals?: unknown
  in?: unknown[]
  notIn?: unknown[]
}

export function visibleWhenOf(value: unknown): VisibleWhen | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const rule = value as Record<string, unknown>
  if (typeof rule.field !== 'string') return undefined
  return {
    field: rule.field,
    equals: rule.equals,
    in: Array.isArray(rule.in) ? rule.in : undefined,
    notIn: Array.isArray(rule.notIn) ? rule.notIn : undefined,
  }
}

/** A regra vale para estes valores? Sem regra, o campo sempre aparece. */
export function ruleHolds(rule: VisibleWhen | undefined, values: Record<string, unknown>): boolean {
  if (!rule) return true
  const value = values[rule.field] ?? null
  if (rule.in) return rule.in.includes(value)
  if (rule.notIn) return !rule.notIn.includes(value)
  return value === (rule.equals ?? null)
}
