import type { Monitor } from '@/stores/monitors'

/**
 * Catálogo dos tipos de checagem suportados pelo backend
 * (`modules/monitoring/checkers`). Este arquivo é a fonte única da verdade da
 * UI: rótulos, máscaras, validações e a tradução entre o formulário e o
 * `configuration` que cada checker espera. Ao adicionar um checker novo no
 * backend, basta descrever o tipo aqui que o formulário se adapta sozinho.
 */

/** Tipos persistidos na coluna `type` do monitor */
export type MonitorApiType = 'ping' | 'http' | 'https' | 'tcp' | 'dns' | 'snmp'

/**
 * Tipos apresentados no formulário. `http` e `https` são a mesma checagem para
 * o usuário — o esquema da URL decide qual valor é gravado.
 */
export type MonitorKind = 'ping' | 'http' | 'tcp' | 'dns' | 'snmp'

/**
 * O checker SNMP faz quatro leituras bem diferentes conforme o `configuration`:
 * disponibilidade (uptime), uso de CPU, uso de memória e estado de interface.
 * Tratamos isso como um sub-tipo explícito para o usuário não precisar saber
 * que "preencher ifIndex" muda o comportamento do monitor.
 */
export type SnmpMode = 'availability' | 'cpu_usage' | 'memory_usage' | 'interface'

export type DnsRecordType = 'A' | 'AAAA' | 'MX' | 'TXT' | 'CNAME' | 'NS'
export type HttpMethod = 'GET' | 'HEAD' | 'POST'
export type SnmpVersion = 'v1' | 'v2c' | 'v3'

/** Regra de validação no formato aceito pelo Vuetify */
export type FieldRule = (value: unknown) => true | string

const IPV4_RE = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/
const IPV6_RE = /^(?=.*:)[0-9a-fA-F:]{2,45}$/
const HOSTNAME_RE =
  /^(?=.{1,253}$)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/
const DOMAIN_RE = /^(?=.{1,253}$)(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$/

export function isIpAddress(value: string): boolean {
  return IPV4_RE.test(value) || IPV6_RE.test(value)
}

export function isHostname(value: string): boolean {
  return HOSTNAME_RE.test(value)
}

export function isDomain(value: string): boolean {
  return DOMAIN_RE.test(value)
}

export function isValidPort(value: unknown): boolean {
  const port = Number(value)
  return Number.isInteger(port) && port >= 1 && port <= 65535
}

/** Resultado da normalização do que o usuário digitou/colou no campo de alvo */
export interface CoercedTarget {
  target: string
  port?: number
}

export interface TargetSpec {
  label: string
  placeholder: string
  hint: string
  icon: string
  /** Filtra caracteres impossíveis para o tipo enquanto o usuário digita */
  sanitize: (raw: string) => string
  /**
   * Interpreta o que foi colado e devolve o alvo no formato que o checker
   * espera (ex.: uma URL completa colada num monitor de ping vira só o host).
   */
  coerce: (raw: string) => CoercedTarget
  rule: FieldRule
}

export interface MonitorKindDefinition {
  kind: MonitorKind
  label: string
  /** Rótulo curto usado em chips e tabelas */
  short: string
  icon: string
  color: string
  /** Frase de 2-4 palavras exibida no card de seleção */
  tagline: string
  /** Explicação do que a checagem faz, exibida abaixo do seletor */
  description: string
  target: TargetSpec
  /** O tipo exige uma porta de destino */
  usesPort: boolean
  defaultPort?: number
  /** O IP do dispositivo selecionado é um alvo natural para o tipo */
  suggestsDeviceIp: boolean
}

/** Remove espaços e caracteres que nunca aparecem em host/IP/hostname */
function sanitizeHost(raw: string): string {
  return raw.replace(/\s+/g, '').replace(/[^a-zA-Z0-9.:_-]/g, '')
}

/**
 * Aceita colagens do mundo real — `https://host/caminho`, `host:8080`,
 * `ping 8.8.8.8` — e extrai host e porta.
 */
function coerceHost(raw: string): CoercedTarget {
  let value = raw.trim().replace(/\s+/g, ' ')

  // "ping 8.8.8.8" / "telnet host 443": fica com o último token relevante
  const commandMatch = value.match(/^(?:ping|telnet|nc|curl|nslookup|dig)\s+(.+)$/i)
  if (commandMatch && commandMatch[1]) {
    const parts = commandMatch[1].split(' ').filter(Boolean)
    value = parts[0] ?? ''
    const trailingPort = parts[1]
    if (trailingPort && isValidPort(trailingPort)) {
      return { target: sanitizeHost(value), port: Number(trailingPort) }
    }
  }

  value = value.replace(/\s/g, '')

  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(value)) {
    try {
      const url = new URL(value)
      return {
        target: url.hostname,
        port: url.port ? Number(url.port) : undefined,
      }
    } catch {
      // Cai no tratamento genérico abaixo
    }
  }

  // Remove caminho/query colados junto do host
  value = value.split('/')[0]?.split('?')[0] ?? ''

  // "host:porta" — ignorado em IPv6, onde os dois-pontos fazem parte do endereço
  const portMatch = value.match(/^([^:]+):(\d{1,5})$/)
  if (portMatch && portMatch[1] && isValidPort(portMatch[2])) {
    return { target: sanitizeHost(portMatch[1]), port: Number(portMatch[2]) }
  }

  return { target: sanitizeHost(value) }
}

function hostRule(emptyMessage: string): FieldRule {
  return (value) => {
    const text = String(value ?? '').trim()
    if (!text) return emptyMessage
    if (isIpAddress(text) || isHostname(text)) return true
    return 'Endereço inválido. Use um IP (ex: 192.168.1.1) ou um hostname (ex: servidor.local)'
  }
}

/** Completa o esquema quando o usuário digita só o domínio */
export function normalizeUrl(raw: string): string {
  const value = raw.trim().replace(/\s+/g, '')
  if (!value) return ''
  if (/^https?:\/\//i.test(value)) return value
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(value)) return value
  return `https://${value}`
}

export const MONITOR_KINDS: MonitorKindDefinition[] = [
  {
    kind: 'ping',
    label: 'Ping (ICMP)',
    short: 'PING',
    icon: 'mdi-lan-pending',
    color: 'primary',
    tagline: 'O host responde?',
    description:
      'Envia pacotes ICMP echo e mede latência e perda. É a checagem padrão para saber se um equipamento está no ar.',
    target: {
      label: 'IP ou hostname do host',
      placeholder: '192.168.1.1',
      hint: 'Aceita IPv4, IPv6 ou nome DNS. Pode colar uma URL — extraímos só o host.',
      icon: 'mdi-ip-network-outline',
      sanitize: sanitizeHost,
      coerce: (raw) => ({ target: coerceHost(raw).target }),
      rule: hostRule('Informe o IP ou hostname que será pingado'),
    },
    usesPort: false,
    suggestsDeviceIp: true,
  },
  {
    kind: 'http',
    label: 'HTTP / HTTPS',
    short: 'HTTP',
    icon: 'mdi-web',
    color: 'indigo',
    tagline: 'Site ou API',
    description:
      'Faz uma requisição HTTP e valida o código de resposta. Use para sites, APIs e páginas de health-check.',
    target: {
      label: 'URL completa',
      placeholder: 'https://meusite.com.br/health',
      hint: 'Se você omitir o esquema, assumimos https://.',
      icon: 'mdi-link-variant',
      sanitize: (raw) => raw.replace(/\s+/g, ''),
      coerce: (raw) => ({ target: normalizeUrl(raw) }),
      rule: (value) => {
        const text = String(value ?? '').trim()
        if (!text) return 'Informe a URL que será monitorada'
        try {
          const url = new URL(normalizeUrl(text))
          if (url.protocol !== 'http:' && url.protocol !== 'https:') {
            return 'Apenas URLs http:// e https:// são suportadas'
          }
          if (!url.hostname.includes('.') && url.hostname !== 'localhost') {
            return 'Informe um domínio completo (ex: https://meusite.com.br)'
          }
          return true
        } catch {
          return 'URL inválida. Exemplo: https://meusite.com.br/health'
        }
      },
    },
    usesPort: false,
    suggestsDeviceIp: false,
  },
  {
    kind: 'tcp',
    label: 'Porta TCP',
    short: 'TCP',
    icon: 'mdi-transit-connection-variant',
    color: 'teal',
    tagline: 'O serviço aceita conexão?',
    description:
      'Abre uma conexão TCP na porta informada e mede o tempo de handshake. Ideal para SSH, banco de dados, RDP e afins.',
    target: {
      label: 'IP ou hostname do serviço',
      placeholder: '192.168.1.10',
      hint: 'Cole "host:porta" que preenchemos a porta automaticamente.',
      icon: 'mdi-server-network',
      sanitize: sanitizeHost,
      coerce: coerceHost,
      rule: hostRule('Informe o host que receberá a conexão TCP'),
    },
    usesPort: true,
    defaultPort: 443,
    suggestsDeviceIp: true,
  },
  {
    kind: 'dns',
    label: 'Resolução DNS',
    short: 'DNS',
    icon: 'mdi-dns-outline',
    color: 'deep-purple',
    tagline: 'O nome resolve?',
    description:
      'Resolve um domínio e mede o tempo de resposta. Detecta queda de servidor DNS e registros removidos ou alterados.',
    target: {
      label: 'Domínio a resolver',
      placeholder: 'meudominio.com.br',
      hint: 'Somente o domínio — sem esquema e sem caminho.',
      icon: 'mdi-web-box',
      sanitize: (raw) => raw.replace(/\s+/g, '').replace(/[^a-zA-Z0-9._-]/g, ''),
      coerce: (raw) => ({ target: coerceHost(raw).target }),
      rule: (value) => {
        const text = String(value ?? '').trim()
        if (!text) return 'Informe o domínio que será resolvido'
        if (isIpAddress(text)) return 'Informe um domínio, não um endereço IP'
        if (!isDomain(text)) return 'Domínio inválido. Exemplo: meudominio.com.br'
        return true
      },
    },
    usesPort: false,
    suggestsDeviceIp: false,
  },
  {
    kind: 'snmp',
    label: 'SNMP',
    short: 'SNMP',
    icon: 'mdi-router-network',
    color: 'orange-darken-2',
    tagline: 'Agente do equipamento',
    description:
      'Consulta o agente SNMP do equipamento. Permite checar disponibilidade, uso de CPU e memória ou o estado de uma interface.',
    target: {
      label: 'IP do agente SNMP',
      placeholder: '192.168.1.1',
      hint: 'Endereço do equipamento com SNMP habilitado.',
      icon: 'mdi-console-network-outline',
      sanitize: sanitizeHost,
      coerce: (raw) => ({ target: coerceHost(raw).target }),
      rule: hostRule('Informe o IP do equipamento com SNMP habilitado'),
    },
    usesPort: false,
    suggestsDeviceIp: true,
  },
]

const KIND_BY_VALUE = new Map(MONITOR_KINDS.map((def) => [def.kind, def]))

export function monitorKind(kind: MonitorKind): MonitorKindDefinition {
  return KIND_BY_VALUE.get(kind) ?? MONITOR_KINDS[0]!
}

export interface SnmpModeDefinition {
  value: SnmpMode
  label: string
  icon: string
  description: string
  /** Leitura percentual (gauge) em vez de checagem up/down */
  isGauge: boolean
}

export const SNMP_MODES: SnmpModeDefinition[] = [
  {
    value: 'availability',
    label: 'Disponibilidade (uptime)',
    icon: 'mdi-check-network-outline',
    description: 'Consulta o uptime do agente — up quando o equipamento responde.',
    isGauge: false,
  },
  {
    value: 'cpu_usage',
    label: 'Uso de CPU',
    icon: 'mdi-cpu-64-bit',
    description: 'Coleta o percentual de uso de CPU. Exibido como medidor, não como up/down.',
    isGauge: true,
  },
  {
    value: 'memory_usage',
    label: 'Uso de memória',
    icon: 'mdi-memory',
    description: 'Coleta o percentual de uso de memória. Exibido como medidor, não como up/down.',
    isGauge: true,
  },
  {
    value: 'interface',
    label: 'Estado de interface',
    icon: 'mdi-ethernet',
    description: 'Acompanha ifOperStatus e a velocidade negociada de uma porta específica.',
    isGauge: false,
  },
]

export const DNS_RECORD_TYPES: Array<{ value: DnsRecordType; title: string; subtitle: string }> = [
  { value: 'A', title: 'A', subtitle: 'Endereço IPv4' },
  { value: 'AAAA', title: 'AAAA', subtitle: 'Endereço IPv6' },
  { value: 'CNAME', title: 'CNAME', subtitle: 'Apelido para outro nome' },
  { value: 'MX', title: 'MX', subtitle: 'Servidores de e-mail' },
  { value: 'TXT', title: 'TXT', subtitle: 'SPF, DKIM e verificações' },
  { value: 'NS', title: 'NS', subtitle: 'Servidores autoritativos' },
]

/** Portas comuns oferecidas como atalho no monitor TCP */
export const COMMON_TCP_PORTS: Array<{ port: number; label: string }> = [
  { port: 22, label: 'SSH' },
  { port: 80, label: 'HTTP' },
  { port: 443, label: 'HTTPS' },
  { port: 3389, label: 'RDP' },
  { port: 3306, label: 'MySQL' },
  { port: 5432, label: 'Postgres' },
  { port: 8291, label: 'Winbox' },
  { port: 161, label: 'SNMP' },
]

export const INTERVAL_PRESETS = [30, 60, 120, 300, 600, 900, 1800, 3600]
export const TIMEOUT_PRESETS = [3, 5, 10, 15, 30, 60]

export function formatSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '—'
  if (seconds < 60) return `${seconds} segundo${seconds === 1 ? '' : 's'}`
  if (seconds < 3600) {
    const minutes = seconds / 60
    const rounded = Number.isInteger(minutes) ? minutes : Number(minutes.toFixed(1))
    return `${rounded} minuto${rounded === 1 ? '' : 's'}`
  }
  const hours = seconds / 3600
  const rounded = Number.isInteger(hours) ? hours : Number(hours.toFixed(1))
  return `${rounded} hora${rounded === 1 ? '' : 's'}`
}

export interface MonitorFormModel {
  deviceId: number | null
  kind: MonitorKind
  name: string
  target: string
  port: number | null
  intervalSeconds: number
  timeoutSeconds: number
  retryCount: number
  enabled: boolean
  // Ping
  packetCount: number
  // HTTP
  httpMethod: HttpMethod
  acceptedStatusCodes: number[]
  validateCertificate: boolean
  // DNS
  recordType: DnsRecordType
  dnsServer: string
  // SNMP
  snmpMode: SnmpMode
  snmpVersion: SnmpVersion
  snmpCommunity: string
  snmpPort: number
  ifIndex: number | null
  ifName: string
}

export const DEFAULT_ACCEPTED_STATUS_CODES = [200, 201, 202, 204, 301, 302]

export function createMonitorForm(deviceId?: number | null): MonitorFormModel {
  return {
    deviceId: deviceId ?? null,
    kind: 'ping',
    name: '',
    target: '',
    port: null,
    intervalSeconds: 60,
    timeoutSeconds: 5,
    retryCount: 3,
    enabled: true,
    packetCount: 3,
    httpMethod: 'GET',
    acceptedStatusCodes: [...DEFAULT_ACCEPTED_STATUS_CODES],
    validateCertificate: true,
    recordType: 'A',
    dnsServer: '',
    snmpMode: 'availability',
    snmpVersion: 'v2c',
    snmpCommunity: 'public',
    snmpPort: 161,
    ifIndex: null,
    ifName: '',
  }
}

/** Traduz o tipo persistido para o tipo apresentado no formulário */
export function resolveKind(type: string | undefined): MonitorKind {
  const normalized = (type || 'ping').toLowerCase()
  if (normalized === 'https') return 'http'
  return KIND_BY_VALUE.has(normalized as MonitorKind) ? (normalized as MonitorKind) : 'ping'
}

export function resolveSnmpMode(configuration: Record<string, unknown> | undefined): SnmpMode {
  const ifIndex = configuration?.ifIndex
  if (ifIndex !== undefined && ifIndex !== null && ifIndex !== '') return 'interface'
  const metric = configuration?.metric
  if (metric === 'cpu_usage') return 'cpu_usage'
  if (metric === 'memory_usage') return 'memory_usage'
  return 'availability'
}

function asNumber(value: unknown, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

/**
 * Reidrata o formulário a partir de um monitor existente. Sem isso, um monitor
 * SNMP editado pela tela de monitores perderia community, métrica e ifIndex,
 * porque o formulário só conhecia host/url/domain.
 */
export function monitorToForm(monitor: Monitor): MonitorFormModel {
  const config = (monitor.configuration || {}) as Record<string, unknown>
  const kind = resolveKind(monitor.type)
  const form = createMonitorForm(monitor.deviceId)

  form.kind = kind
  form.name = monitor.name || ''
  form.intervalSeconds = asNumber(monitor.intervalSeconds, 60)
  form.timeoutSeconds = asNumber(monitor.timeoutSeconds, 5)
  form.retryCount = Number.isFinite(Number(monitor.retryCount)) ? Number(monitor.retryCount) : 3
  form.enabled = monitor.isEnabled ?? monitor.enabled ?? true

  form.target =
    monitor.target || ((config.host || config.url || config.domain || '') as string) || ''

  switch (kind) {
    case 'ping':
      form.packetCount = asNumber(config.packetCount, 3)
      break
    case 'http':
      form.httpMethod = ((config.method as HttpMethod) || 'GET').toUpperCase() as HttpMethod
      form.acceptedStatusCodes = Array.isArray(config.acceptedStatusCodes)
        ? (config.acceptedStatusCodes as unknown[]).map(Number).filter(Number.isFinite)
        : [...DEFAULT_ACCEPTED_STATUS_CODES]
      form.validateCertificate = config.validateCertificate !== false
      break
    case 'tcp':
      form.port = asNumber(config.port ?? monitor.port, 443)
      break
    case 'dns':
      form.recordType = ((config.recordType as DnsRecordType) || 'A').toUpperCase() as DnsRecordType
      form.dnsServer = (config.dnsServer as string) || ''
      break
    case 'snmp':
      form.snmpMode = resolveSnmpMode(config)
      form.snmpVersion = (config.version as SnmpVersion) || 'v2c'
      form.snmpCommunity = (config.community as string) || 'public'
      form.snmpPort = asNumber(config.port, 161)
      form.ifIndex =
        config.ifIndex === undefined || config.ifIndex === null ? null : Number(config.ifIndex)
      form.ifName = (config.ifName as string) || ''
      break
  }

  return form
}

/** Tipo gravado no monitor — HTTPS é derivado do esquema da URL */
export function apiTypeFor(form: MonitorFormModel): MonitorApiType {
  if (form.kind !== 'http') return form.kind
  return /^https:\/\//i.test(normalizeUrl(form.target)) ? 'https' : 'http'
}

/** Monta o `configuration` no formato esperado pelo checker correspondente */
export function buildConfiguration(form: MonitorFormModel): Record<string, unknown> {
  const timeoutMs = Math.max(1, asNumber(form.timeoutSeconds, 5)) * 1000
  const target = form.target.trim()

  switch (form.kind) {
    case 'ping':
      return {
        host: target,
        packetCount: form.packetCount,
        timeoutMs,
      }
    case 'http':
      return {
        url: normalizeUrl(target),
        method: form.httpMethod,
        acceptedStatusCodes: form.acceptedStatusCodes.length
          ? form.acceptedStatusCodes
          : [...DEFAULT_ACCEPTED_STATUS_CODES],
        validateCertificate: form.validateCertificate,
        timeoutMs,
      }
    case 'tcp':
      return {
        host: target,
        port: Number(form.port) || 443,
        timeoutMs,
      }
    case 'dns': {
      const config: Record<string, unknown> = {
        domain: target,
        recordType: form.recordType,
        timeoutMs,
      }
      if (form.dnsServer.trim()) config.dnsServer = form.dnsServer.trim()
      return config
    }
    case 'snmp': {
      const config: Record<string, unknown> = {
        host: target,
        version: form.snmpVersion,
        community: form.snmpCommunity.trim() || 'public',
        port: Number(form.snmpPort) || 161,
        timeoutMs,
      }
      if (form.snmpMode === 'cpu_usage' || form.snmpMode === 'memory_usage') {
        config.metric = form.snmpMode
      }
      if (form.snmpMode === 'interface' && form.ifIndex !== null) {
        config.ifIndex = Number(form.ifIndex)
        if (form.ifName) config.ifName = form.ifName
      }
      return config
    }
    default:
      return { host: target, timeoutMs }
  }
}

export function formToPayload(form: MonitorFormModel): Record<string, unknown> {
  const configuration = buildConfiguration(form)

  return {
    deviceId: form.deviceId,
    type: apiTypeFor(form),
    name: form.name.trim(),
    configuration,
    target: form.target.trim(),
    port: form.kind === 'tcp' ? Number(form.port) || null : null,
    intervalSeconds: form.intervalSeconds,
    timeoutSeconds: form.timeoutSeconds,
    retryCount: form.retryCount,
    enabled: form.enabled,
    isEnabled: form.enabled,
  }
}

/** Nome sugerido enquanto o usuário não digitar o seu */
export function suggestMonitorName(form: MonitorFormModel, deviceName?: string): string {
  const reference = deviceName || form.target.trim()
  if (!reference) return ''

  switch (form.kind) {
    case 'ping':
      return `Ping — ${reference}`
    case 'http': {
      let host = form.target.trim()
      try {
        host = new URL(normalizeUrl(form.target)).host
      } catch {
        // Mantém o texto digitado enquanto a URL ainda está incompleta
      }
      return `HTTP — ${host || reference}`
    }
    case 'tcp':
      return `TCP ${form.port || ''} — ${reference}`.replace('  ', ' ')
    case 'dns':
      return `DNS ${form.recordType} — ${form.target.trim() || reference}`
    case 'snmp':
      switch (form.snmpMode) {
        case 'cpu_usage':
          return `Uso de CPU — ${reference}`
        case 'memory_usage':
          return `Uso de memória — ${reference}`
        case 'interface':
          return form.ifName
            ? `Interface ${form.ifName} — ${reference}`
            : `Interface — ${reference}`
        default:
          return `SNMP — ${reference}`
      }
    default:
      return reference
  }
}

/** Frase de conferência exibida no rodapé do formulário */
export function describeMonitor(form: MonitorFormModel): string {
  const definition = monitorKind(form.kind)
  const every = `a cada ${formatSeconds(form.intervalSeconds)}`
  const target = form.target.trim() || '—'

  switch (form.kind) {
    case 'ping':
      return `${form.packetCount} pacotes ICMP para ${target}, ${every}.`
    case 'http':
      return `${form.httpMethod} em ${normalizeUrl(target)}, ${every}, aceitando ${form.acceptedStatusCodes.join(', ')}.`
    case 'tcp':
      return `Conexão TCP em ${target}:${form.port || '—'}, ${every}.`
    case 'dns':
      return `Consulta ${form.recordType} de ${target}${
        form.dnsServer ? ` em ${form.dnsServer}` : ''
      }, ${every}.`
    case 'snmp': {
      const mode = SNMP_MODES.find((m) => m.value === form.snmpMode)
      const suffix =
        form.snmpMode === 'interface' && form.ifIndex !== null
          ? ` (interface #${form.ifIndex}${form.ifName ? ` — ${form.ifName}` : ''})`
          : ''
      return `${mode?.label ?? 'SNMP'}${suffix} via ${form.snmpVersion} em ${target}, ${every}.`
    }
    default:
      return `${definition.label} em ${target}, ${every}.`
  }
}

/** Validação completa do formulário — usada para habilitar o botão Salvar */
export function validateMonitorForm(form: MonitorFormModel): string[] {
  const errors: string[] = []
  const definition = monitorKind(form.kind)

  if (!form.deviceId) errors.push('Selecione o dispositivo associado')
  if (!form.name.trim()) errors.push('Informe um nome para o monitor')

  const targetCheck = definition.target.rule(form.target)
  if (targetCheck !== true) errors.push(targetCheck)

  if (definition.usesPort && !isValidPort(form.port)) {
    errors.push('Informe uma porta entre 1 e 65535')
  }

  if (form.kind === 'snmp') {
    if (
      form.snmpMode === 'interface' &&
      (form.ifIndex === null || !Number.isFinite(form.ifIndex))
    ) {
      errors.push('Selecione a interface que será monitorada')
    }
    if (!isValidPort(form.snmpPort)) errors.push('Porta SNMP inválida')
  }

  if (form.kind === 'dns' && form.dnsServer.trim() && !isIpAddress(form.dnsServer.trim())) {
    errors.push('Servidor DNS deve ser um endereço IP')
  }

  if (!(form.intervalSeconds >= 10)) errors.push('O intervalo mínimo é de 10 segundos')
  if (!(form.timeoutSeconds >= 1)) errors.push('O timeout mínimo é de 1 segundo')
  if (form.timeoutSeconds >= form.intervalSeconds) {
    errors.push('O timeout precisa ser menor que o intervalo entre checagens')
  }

  return errors
}
