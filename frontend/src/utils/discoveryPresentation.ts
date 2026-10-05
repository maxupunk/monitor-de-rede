/**
 * Leitura do que a descoberta contou de um host, para a tela.
 *
 * O mesmo host chega de dois jeitos: ao vivo, pelo snapshot da varredura
 * (`data.<bloco>`), e persistido, no histórico (`data.details.<bloco>`). Tudo
 * aqui aceita os dois — a tela não precisa saber de onde ele veio.
 */
import type { IdentitySuggestion } from '@/bindings/IdentitySuggestion'
import { deviceTypeMeta, normalizeDeviceType, type DeviceTypePresentation } from './deviceTypes'

/** O mínimo de um host descoberto que a apresentação lê. */
export interface DiscoveredHostLike {
  ipAddress?: string
  hostname?: string | null
  mdnsName?: string | null
  vendor?: string | null
  deviceType?: string | null
  macAddress?: string | null
  openPorts?: number[] | null
  confidence?: number
  data?: Record<string, unknown> | null
}

export interface DiscoveryIdentity {
  operatingSystem?: string
  label?: string
  source?: string
  reason?: string
  sysDescr?: string
  sysObjectId?: string
  sysName?: string
  hardwareVendor?: string
  hardwareModel?: string
}

export interface DiscoveryWebPage {
  port: number
  status: number
  server?: string
  title?: string
  realm?: string
  location?: string
}

export interface DiscoveryUpnp {
  server?: string
  location?: string
  types?: string[]
  friendlyName?: string
  manufacturer?: string
  modelName?: string
  modelDescription?: string
  deviceType?: string
}

export interface DiscoveryMdns {
  services?: string[]
  instances?: string[]
  model?: string
}

export interface DiscoveryNetbios {
  name?: string
  workgroup?: string
}

export interface DiscoveryClassification {
  deviceType?: string
  confidence?: number
  reasons?: string[]
  alternative?: string
}

type Block = Record<string, unknown>

function isBlock(value: unknown): value is Block {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** Um campo dos scanners, do snapshot ao vivo ou do resultado persistido. */
function detail(host: DiscoveredHostLike | null | undefined, key: string): unknown {
  const data = host?.data
  if (!isBlock(data)) return undefined
  const container = isBlock(data.details) ? data.details : data
  return container[key]
}

function block(host: DiscoveredHostLike | null | undefined, key: string): Block | null {
  const value = detail(host, key)
  return isBlock(value) ? value : null
}

function list(host: DiscoveredHostLike | null | undefined, key: string): unknown[] {
  const value = detail(host, key)
  return Array.isArray(value) ? value : []
}

function text(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null
}

export function discoveryIdentity(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'identity') as DiscoveryIdentity | null
}

/** O palpite do Laya (tipo e sistema), quando a heurística ficou em dúvida. */
export function discoveryLaya(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'laya') as IdentitySuggestion | null
}

export function discoveryWebPage(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'http') as DiscoveryWebPage | null
}

export function discoveryUpnp(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'ssdp') as DiscoveryUpnp | null
}

export function discoveryMdns(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'mdns') as DiscoveryMdns | null
}

export function discoveryNetbios(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'netbios') as DiscoveryNetbios | null
}

export function discoveryClassification(host: DiscoveredHostLike | null | undefined) {
  return block(host, 'classification') as DiscoveryClassification | null
}

/**
 * O nome mais específico do aparelho: o que ele diz de si por SNMP, o do DNS
 * ou NetBIOS, o nome dado pelo dono (mDNS/UPnP) e, por fim, o nome mDNS.
 */
export function discoveryDeviceName(host: DiscoveredHostLike | null | undefined): string | null {
  if (!host) return null
  return (
    text(discoveryIdentity(host)?.sysName) ??
    text(host.hostname) ??
    text(discoveryUpnp(host)?.friendlyName) ??
    text(discoveryMdns(host)?.instances?.[0]) ??
    text(host.mdnsName) ??
    null
  )
}

/** Uma linha que descreve o aparelho: modelo, título da página ou sysDescr. */
export function discoveryDescription(host: DiscoveredHostLike | null | undefined): string | null {
  const upnp = discoveryUpnp(host)
  const model = [text(upnp?.manufacturer), text(upnp?.modelName)].filter(Boolean).join(' ')
  return (
    text(model) ??
    text(discoveryMdns(host)?.model) ??
    text(discoveryIdentity(host)?.sysDescr) ??
    text(discoveryWebPage(host)?.title) ??
    text(discoveryWebPage(host)?.realm) ??
    null
  )
}

/** Fabricante do MAC, do SNMP ou da descrição UPnP. */
export function discoveryVendor(host: DiscoveredHostLike | null | undefined): string | null {
  return (
    text(host?.vendor) ??
    text(discoveryIdentity(host)?.hardwareVendor) ??
    text(discoveryUpnp(host)?.manufacturer) ??
    null
  )
}

/**
 * Fabricante para mostrar, ou o porquê de não haver: MAC aleatório não tem
 * dono no registro do IEEE, e "não identificado" sozinho parece falha.
 */
export function discoveryVendorLabel(host: DiscoveredHostLike | null | undefined): string {
  const vendor = discoveryVendor(host)
  if (vendor) return vendor
  return detail(host, 'macPrivate') === true
    ? 'MAC aleatório (privacidade)'
    : 'Fabricante não identificado'
}

export function discoveryTypeMeta(host: DiscoveredHostLike | null | undefined) {
  return deviceTypeMeta(host?.deviceType)
}

/** Tipo pronto para o select do cadastro; o gateway da rede é roteador. */
export function discoveryRegistrationType(host: DiscoveredHostLike, isGateway: boolean) {
  return isGateway ? 'router' : normalizeDeviceType(host.deviceType)
}

/** Por que a descoberta acha que o aparelho é desse tipo. */
export function discoveryReasons(host: DiscoveredHostLike | null | undefined): string[] {
  return (discoveryClassification(host)?.reasons ?? []).filter(
    (reason): reason is string => typeof reason === 'string'
  )
}

/** O segundo tipo mais provável, quando ficou perto do vencedor. */
export function discoveryAlternative(
  host: DiscoveredHostLike | null | undefined
): DeviceTypePresentation | null {
  const alternative = discoveryClassification(host)?.alternative
  return alternative ? deviceTypeMeta(alternative) : null
}

export function discoverySnmpVersion(host: DiscoveredHostLike | null | undefined): string | null {
  const snmp = block(host, 'snmp')
  if (snmp?.detected) return text(snmp.version) ?? ''
  return discoveryIdentity(host)?.source === 'snmp' ? '' : null
}

export function discoveryHasSnmp(host: DiscoveredHostLike | null | undefined): boolean {
  return discoverySnmpVersion(host) !== null
}

/** As portas abertas, das duas formas em que chegam. */
export function discoveryOpenPorts(host: DiscoveredHostLike | null | undefined): number[] {
  if (Array.isArray(host?.openPorts) && host.openPorts.length > 0) return host.openPorts
  const persisted = host?.data?.openPorts
  return Array.isArray(persisted)
    ? persisted.filter((port): port is number => typeof port === 'number')
    : []
}

/** Nome do serviço de cada porta, como o backend rotulou. */
export function discoveryPortLabel(
  host: DiscoveredHostLike | null | undefined,
  port: number
): string | null {
  return text(block(host, 'services')?.[String(port)])
}

const SOURCE_LABELS: Record<string, string> = {
  icmp: 'Ping',
  arp: 'ARP',
  tcp: 'TCP',
  snmp: 'SNMP',
  mdns: 'mDNS',
  ssdp: 'UPnP',
  http: 'Web',
  dns: 'DNS',
  netbios: 'NetBIOS',
}

/** Quem enxergou o host, em rótulos curtos. */
export function discoverySources(host: DiscoveredHostLike | null | undefined): string[] {
  return list(host, 'sources')
    .filter((source): source is string => typeof source === 'string')
    .map((source) => SOURCE_LABELS[source] ?? source)
}

/** Texto em que a busca da tela procura. */
export function discoverySearchText(host: DiscoveredHostLike): string {
  return [
    host.ipAddress,
    host.macAddress,
    discoveryDeviceName(host),
    discoveryVendor(host),
    discoveryDescription(host),
    discoveryTypeMeta(host).label,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase()
}

/** Ordena IPv4 numericamente (10.0.0.9 antes de 10.0.0.10). */
export function compareIpAddresses(a: string, b: string): number {
  const parts = (ip: string) => ip.split('.').map((part) => Number.parseInt(part, 10))
  const left = parts(a)
  const right = parts(b)
  if (left.length === 4 && right.length === 4 && [...left, ...right].every(Number.isFinite)) {
    for (let index = 0; index < 4; index += 1) {
      if (left[index] !== right[index]) return left[index] - right[index]
    }
    return 0
  }
  return a.localeCompare(b)
}

/** Cor da confiança na classificação: forte, razoável ou fraca. */
export function discoveryConfidenceColor(confidence?: number | null): string {
  const value = confidence ?? 0
  if (value >= 70) return 'success'
  if (value >= 45) return 'info'
  return 'warning'
}
