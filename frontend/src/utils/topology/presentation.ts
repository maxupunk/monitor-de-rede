/**
 * Apresentação da topologia — fonte única de rótulos, ícones e cores de nós e
 * enlaces. O mapa (Cytoscape), o inspetor, a legenda, o menu de contexto e o
 * diálogo de enlace leem daqui: nenhum deles tem tabela própria (DRY).
 *
 * Cores são tokens do tema Vuetify (`success`, `error`…), nunca hex soltos —
 * o estilo do canvas resolve o token no tema ativo (claro ou escuro).
 */
import {
  mdiAccessPoint,
  mdiBatteryChargingHigh,
  mdiCctv,
  mdiCellphone,
  mdiDesktopTowerMonitor,
  mdiDevices,
  mdiHelpNetworkOutline,
  mdiHomeAutomation,
  mdiHub,
  mdiMonitorDashboard,
  mdiNas,
  mdiPhoneVoip,
  mdiPrinter,
  mdiRouterNetwork,
  mdiServer,
  mdiSwitch,
  mdiTelevisionClassic,
  mdiWallFire,
  mdiWeb,
} from '@mdi/js'
import { DEVICE_TYPE_OPTIONS, deviceTypeMeta } from '@/utils/deviceTypes'
import { formatBps, formatLinkSpeed } from '@/utils/formatters'
import type { TopologyEdge, TopologyNode } from '@/stores/topology'

export type ThemeColorToken = 'primary' | 'secondary' | 'success' | 'error' | 'warning' | 'info'

// --- Nós -------------------------------------------------------------------

export type NodeStatusKey = 'online' | 'offline' | 'warning' | 'unknown'

export interface StatusPresentation {
  label: string
  color: ThemeColorToken
  icon: string
}

export const NODE_STATUS: Record<NodeStatusKey, StatusPresentation> = {
  online: { label: 'Online', color: 'success', icon: 'mdi-check-circle' },
  offline: { label: 'Offline', color: 'error', icon: 'mdi-close-circle' },
  warning: { label: 'Alerta', color: 'warning', icon: 'mdi-alert' },
  unknown: { label: 'Sem dados', color: 'info', icon: 'mdi-help-circle' },
}

export function nodeStatusKey(status?: string | null): NodeStatusKey {
  const key = (status ?? '').toLowerCase()
  if (key === 'online' || key === 'up') return 'online'
  if (key === 'offline' || key === 'down') return 'offline'
  if (key === 'warning' || key === 'degraded') return 'warning'
  return 'unknown'
}

export function nodeStatusPresentation(status?: string | null): StatusPresentation {
  return NODE_STATUS[nodeStatusKey(status)]
}

const UNMANAGED_TYPES = new Set(['unmanaged_switch', 'hub'])

/** Chave de ícone do nó — também o seletor do estilo no canvas. */
export type NodeIconKey = ReturnType<typeof deviceTypeMeta>['id'] | 'unmanaged_switch' | 'system'

export function nodeIconKey(node: Pick<TopologyNode, 'type' | 'isSystem'>): NodeIconKey {
  if (node.isSystem) return 'system'
  const raw = (node.type ?? '').toLowerCase()
  if (UNMANAGED_TYPES.has(raw)) return 'unmanaged_switch'
  return deviceTypeMeta(raw).id
}

/** Caminho SVG (24×24) de cada ícone — o canvas não usa a fonte de ícones. */
export const NODE_ICON_PATHS: Record<NodeIconKey, string> = {
  router: mdiRouterNetwork,
  switch: mdiSwitch,
  unmanaged_switch: mdiHub,
  firewall: mdiWallFire,
  ap: mdiAccessPoint,
  server: mdiServer,
  nas: mdiNas,
  workstation: mdiDesktopTowerMonitor,
  mobile: mdiCellphone,
  printer: mdiPrinter,
  camera: mdiCctv,
  media: mdiTelevisionClassic,
  iot: mdiHomeAutomation,
  voip: mdiPhoneVoip,
  ups: mdiBatteryChargingHigh,
  other: mdiDevices,
  web_device: mdiWeb,
  unknown: mdiHelpNetworkOutline,
  system: mdiMonitorDashboard,
}

/** Classe `mdi-*` do ícone (para `v-icon`). */
export function nodeIcon(node: Pick<TopologyNode, 'type' | 'isSystem'>): string {
  const key = nodeIconKey(node)
  if (key === 'system') return 'mdi-monitor-dashboard'
  if (key === 'unmanaged_switch') return 'mdi-hub'
  return deviceTypeMeta(node.type).icon
}

export function nodeTypeLabel(node: Pick<TopologyNode, 'type' | 'isSystem'>): string {
  if (node.isSystem) return 'Servidor NetMonitor'
  if (UNMANAGED_TYPES.has((node.type ?? '').toLowerCase())) return 'Switch não gerenciável'
  return deviceTypeMeta(node.type).label
}

export function isUnmanagedSwitch(node: Pick<TopologyNode, 'type'>): boolean {
  return UNMANAGED_TYPES.has((node.type ?? '').toLowerCase())
}

/** Categoria usada no filtro por tipo (`unmanaged_switch` conta como switch). */
export function nodeCategory(node: Pick<TopologyNode, 'type'>): string {
  return deviceTypeMeta(node.type).id
}

export const NODE_CATEGORY_OPTIONS: ReadonlyArray<{ value: string; title: string; icon: string }> =
  [
    ...DEVICE_TYPE_OPTIONS.map((option) => ({
      value: option.value as string,
      title: option.title as string,
      icon: deviceTypeMeta(option.value).icon,
    })),
    { value: 'web_device', title: 'Dispositivo web', icon: 'mdi-web' },
    { value: 'unknown', title: 'Desconhecido', icon: 'mdi-help-network-outline' },
  ]

export function nodeIp(node: Pick<TopologyNode, 'ipAddress'>): string | null {
  return node.ipAddress || null
}

// --- Enlaces ---------------------------------------------------------------

/** Meio físico/lógico do enlace — o que a legenda chama de "tipo". */
export type LinkKind = 'ethernet' | 'fiber' | 'wireless' | 'vpn' | 'inferred' | 'parent'

export interface LinkKindPresentation {
  label: string
  icon: string
  color: ThemeColorToken
  /** Como a linha aparece no mapa (a legenda descreve o mesmo). */
  line: 'solid' | 'dashed' | 'dotted'
}

export const LINK_KINDS: Record<LinkKind, LinkKindPresentation> = {
  ethernet: { label: 'Cabo Ethernet', icon: 'mdi-ethernet', color: 'primary', line: 'solid' },
  fiber: { label: 'Fibra óptica', icon: 'mdi-laser-pointer', color: 'secondary', line: 'solid' },
  wireless: { label: 'Sem fio / rádio', icon: 'mdi-wifi', color: 'info', line: 'dashed' },
  vpn: { label: 'Túnel VPN', icon: 'mdi-shield-lock', color: 'warning', line: 'dashed' },
  inferred: {
    label: 'Inferido (sub-rede)',
    icon: 'mdi-lan-pending',
    color: 'info',
    line: 'dashed',
  },
  parent: { label: 'Hierarquia (pai)', icon: 'mdi-sitemap', color: 'secondary', line: 'dotted' },
}

export function linkKind(linkType?: string | null): LinkKind {
  switch ((linkType ?? '').toLowerCase()) {
    case 'fiber':
      return 'fiber'
    case 'wireless':
      return 'wireless'
    case 'vpn':
      return 'vpn'
    case 'inferred':
    case 'subnet':
      return 'inferred'
    case 'parent':
      return 'parent'
    default:
      // manual, snmp, lldp, cdp: cabo físico.
      return 'ethernet'
  }
}

export function linkKindPresentation(linkType?: string | null): LinkKindPresentation {
  return LINK_KINDS[linkKind(linkType)]
}

/** Tipos que o operador escolhe ao criar/editar um enlace. */
export const LINK_TYPE_OPTIONS: ReadonlyArray<{
  value: string
  label: string
  icon: string
  color: ThemeColorToken
}> = [
  { value: 'manual', ...pick('ethernet') },
  { value: 'fiber', ...pick('fiber') },
  { value: 'wireless', ...pick('wireless') },
  { value: 'vpn', ...pick('vpn') },
]

function pick(kind: LinkKind) {
  const { label, icon, color } = LINK_KINDS[kind]
  return { label, icon, color }
}

export function discoveryLabel(method?: string | null): string {
  switch ((method ?? '').toLowerCase()) {
    case 'lldp':
      return 'Descoberto por LLDP'
    case 'cdp':
      return 'Descoberto por CDP'
    case 'subnet_inference':
      return 'Inferido pela sub-rede'
    case 'user_defined':
      return 'Definido manualmente'
    case 'parent_hierarchy':
      return 'Hierarquia (dispositivo pai)'
    case '':
      return 'Origem desconhecida'
    default:
      return 'Descoberto automaticamente'
  }
}

export function isVirtualEdge(edge: Pick<TopologyEdge, 'id' | 'linkType'>): boolean {
  return edge.id < 0 || linkKind(edge.linkType) === 'parent'
}

/** Descoberto/inferido e ainda não confirmado pelo operador. */
export function canConfirmEdge(
  edge: Pick<TopologyEdge, 'id' | 'linkType' | 'confirmed' | 'discoveryMethod'>
): boolean {
  return !isVirtualEdge(edge) && !edge.confirmed && (edge.discoveryMethod ?? '') !== 'user_defined'
}

export type EdgeStatusKey = 'up' | 'down' | 'unknown'

export const EDGE_STATUS: Record<EdgeStatusKey, StatusPresentation> = {
  up: { label: 'Ativo', color: 'success', icon: 'mdi-arrow-up-bold-circle' },
  down: { label: 'Fora do ar', color: 'error', icon: 'mdi-arrow-down-bold-circle' },
  unknown: { label: 'Sem dados', color: 'info', icon: 'mdi-help-circle' },
}

export function edgeStatusKey(
  edge: Pick<TopologyEdge, 'status' | 'sourceInterfaceStatus' | 'targetInterfaceStatus'>
): EdgeStatusKey {
  const status = (edge.status ?? '').toLowerCase()
  if (status === 'down') return 'down'
  if (edge.sourceInterfaceStatus === 'down' || edge.targetInterfaceStatus === 'down') return 'down'
  if (status === 'up') return 'up'
  return 'unknown'
}

/** Velocidade nominal do enlace: a do backend ou a menor das duas portas. */
export function edgeSpeed(
  edge: Pick<TopologyEdge, 'speedBps' | 'sourceInterfaceSpeed' | 'targetInterfaceSpeed'>
): number | null {
  if (edge.speedBps && edge.speedBps > 0) return edge.speedBps
  const speeds = [edge.sourceInterfaceSpeed, edge.targetInterfaceSpeed].filter(
    (value): value is number => typeof value === 'number' && value > 0
  )
  return speeds.length > 0 ? Math.min(...speeds) : null
}

export type SpeedTier = 'unknown' | 'slow' | 'gigabit' | 'fast'

/** ≤ 100 Mbps fino, 1 Gbps médio, ≥ 10 Gbps grosso. */
export function speedTier(bps: number | null): SpeedTier {
  if (!bps || bps <= 0) return 'unknown'
  if (bps <= 100_000_000) return 'slow'
  if (bps < 10_000_000_000) return 'gigabit'
  return 'fast'
}

/** Uso 0..1 — o do backend ou o maior sentido sobre a velocidade. */
export function edgeUtilization(edge: TopologyEdge): number | null {
  if (typeof edge.utilization === 'number' && Number.isFinite(edge.utilization)) {
    return Math.max(0, Math.min(1, edge.utilization))
  }
  const speed = edgeSpeed(edge)
  const peak = Math.max(edge.inBps ?? -1, edge.outBps ?? -1)
  if (!speed || peak < 0) return null
  return Math.max(0, Math.min(1, peak / speed))
}

export type UtilizationLevel = 'normal' | 'high' | 'critical'

export const UTILIZATION_LEVELS: Record<
  UtilizationLevel,
  { label: string; color: ThemeColorToken; threshold: number }
> = {
  normal: { label: 'Uso normal (< 70%)', color: 'success', threshold: 0 },
  high: { label: 'Uso alto (≥ 70%)', color: 'warning', threshold: 0.7 },
  critical: { label: 'Saturado (≥ 90%)', color: 'error', threshold: 0.9 },
}

export function utilizationLevel(utilization: number | null): UtilizationLevel | null {
  if (utilization === null) return null
  if (utilization >= UTILIZATION_LEVELS.critical.threshold) return 'critical'
  if (utilization >= UTILIZATION_LEVELS.high.threshold) return 'high'
  return 'normal'
}

export function hasTraffic(edge: Pick<TopologyEdge, 'inBps' | 'outBps'>): boolean {
  return (
    (edge.inBps !== null && edge.inBps !== undefined) ||
    (edge.outBps !== null && edge.outBps !== undefined)
  )
}

/** "↓ 15 Mbps ↑ 45 Mbps" — `null` quando o enlace não tem medição. */
export function edgeTrafficLabel(edge: Pick<TopologyEdge, 'inBps' | 'outBps'>): string | null {
  if (!hasTraffic(edge)) return null
  return `↓ ${formatBps(edge.inBps ?? 0)} ↑ ${formatBps(edge.outBps ?? 0)}`
}

export function edgeSpeedLabel(edge: TopologyEdge): string {
  return formatLinkSpeed(edgeSpeed(edge))
}

/** 0–100 (backend) ou 0..1 — devolve 0..100 inteiro. */
export function confidencePercent(confidence?: number | null): number | null {
  if (confidence === null || confidence === undefined || !Number.isFinite(confidence)) return null
  const value = confidence <= 1 ? confidence * 100 : confidence
  return Math.round(Math.max(0, Math.min(100, value)))
}

export function edgeEndpoints(
  edge: Pick<TopologyEdge, 'source' | 'target' | 'sourceDeviceId' | 'targetDeviceId'>
): { source: number; target: number } {
  return {
    source: edge.sourceDeviceId ?? edge.source,
    target: edge.targetDeviceId ?? edge.target,
  }
}

/** Uso a partir do qual a linha de saúde do nó ganha o alerta. */
export const HEALTH_HIGH_PERCENT = 90

/**
 * A linha de saúde sob o nome do nó: "CPU 37% · RAM 62%" (só o que há de
 * coleta recente). Uso alto ganha "⚠ " na frente — o anel do nó continua sendo
 * o estado do dispositivo. `null` sem coleta.
 */
export function nodeHealthText(
  node: Partial<Pick<TopologyNode, 'cpuPercent' | 'memoryPercent'>>
): string | null {
  const parts: string[] = []
  const values: number[] = []
  if (typeof node.cpuPercent === 'number') {
    parts.push(`CPU ${Math.round(node.cpuPercent)}%`)
    values.push(node.cpuPercent)
  }
  if (typeof node.memoryPercent === 'number') {
    parts.push(`RAM ${Math.round(node.memoryPercent)}%`)
    values.push(node.memoryPercent)
  }
  if (parts.length === 0) return null
  const high = values.some((value) => value >= HEALTH_HIGH_PERCENT)
  return `${high ? '⚠ ' : ''}${parts.join(' · ')}`
}
