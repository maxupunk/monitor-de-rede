/**
 * Apresentação da telemetria Wi-Fi: cores, rótulos, filtros e o achatamento
 * dos snapshots em linhas. A classificação (faixas e saúde) vem pronta do
 * backend (`services::wifi::quality`) — aqui só se escolhe como mostrar.
 */
import type { WifiBand } from '@/bindings/WifiBand'
import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import type { WifiEnvironment } from '@/bindings/WifiEnvironment'
import type { WifiGrade } from '@/bindings/WifiGrade'
import type { WifiHealth } from '@/bindings/WifiHealth'
import type { WifiReadStatus } from '@/bindings/WifiReadStatus'
import type { WifiRoamEvent } from '@/bindings/WifiRoamEvent'
import type { WifiStation } from '@/bindings/WifiStation'
import type { WifiWatchState } from '@/bindings/WifiWatchState'
import type { WifiWatchedSnapshot } from '@/bindings/WifiWatchedSnapshot'
import { formatDistanceRange, formatPercent } from '@/utils/formatters'
import { formatOutputValue } from '@/utils/pluginPresentation'

/** Uma estação com o equipamento onde está conectada. */
export interface WifiStationRow extends WifiStation {
  deviceId: number
  deviceName: string
  canPing: boolean
}

export const GRADE_COLOR: Record<WifiGrade, string> = {
  excellent: 'success',
  good: 'success',
  fair: 'warning',
  poor: 'error',
  critical: 'error',
}

export const GRADE_LABEL: Record<WifiGrade, string> = {
  excellent: 'Excelente',
  good: 'Bom',
  fair: 'Atenção',
  poor: 'Ruim',
  critical: 'Crítico',
}

const SIGNAL_ICON: Record<WifiGrade, string> = {
  excellent: 'mdi-wifi-strength-4',
  good: 'mdi-wifi-strength-3',
  fair: 'mdi-wifi-strength-2',
  poor: 'mdi-wifi-strength-1',
  critical: 'mdi-wifi-strength-alert-outline',
}

export function signalIcon(grade: WifiGrade): string {
  return SIGNAL_ICON[grade]
}

export interface HealthPresentation {
  label: string
  color: string
  icon: string
}

export const HEALTH: Record<WifiHealth, HealthPresentation> = {
  critical: { label: 'Sinal ruim', color: 'error', icon: 'mdi-wifi-alert' },
  high_retries: { label: 'Interferência', color: 'error', icon: 'mdi-sine-wave' },
  fair: { label: 'Atenção', color: 'warning', icon: 'mdi-alert-outline' },
  healthy: { label: 'Saudável', color: 'success', icon: 'mdi-check-circle-outline' },
  power_save: { label: 'Em repouso', color: 'info', icon: 'mdi-sleep' },
}

export const BAND_LABEL: Record<WifiBand, string> = {
  '2g': '2,4 GHz',
  '5g': '5 GHz',
  '6g': '6 GHz',
}

export const BAND_COLOR: Record<WifiBand, string> = {
  '2g': 'primary',
  '5g': 'secondary',
  '6g': 'info',
}

/** Sinal ruim, interferência ou atenção — o repouso não é problema da rede. */
export function needsAttention(health: WifiHealth): boolean {
  return health === 'critical' || health === 'high_retries' || health === 'fair'
}

/** Como chamar a estação: o nome do DHCP, o fabricante ou o que se sabe dela. */
export function stationName(station: WifiStation): string {
  if (station.hostname) return station.hostname
  if (station.vendor) return `Aparelho ${station.vendor}`
  return station.randomizedMac ? 'Aparelho com MAC privado' : 'Aparelho sem nome'
}

/** A aba Wi-Fi do roteador onde a estação está. */
export function wifiTabOf(row: WifiStationRow) {
  return { path: `/devices/${row.deviceId}`, query: { tab: 'wifi' } }
}

/** Por que não dá para medir a latência deste cliente; vazio quando dá. */
export function pingUnavailableReason(row: WifiStationRow): string {
  if (!row.canPing) return 'O plugin deste roteador não mede latência.'
  if (!row.ip) return 'O roteador não sabe o IP deste cliente.'
  return ''
}

/** Retransmissão e há quanto tempo está conectado, quando o AP informa. */
export function stationDetails(station: WifiStation): string {
  const parts: string[] = []
  if (station.txRetriesPct !== null) {
    parts.push(`${formatPercent(station.txRetriesPct)} de retransmissão`)
  }
  if (station.connectedS !== null) {
    parts.push(`conectado há ${formatOutputValue(station.connectedS, 'uptime')}`)
  }
  return parts.join(' · ')
}

/** Canal e largura, quando o AP informa: "canal 36 · 80 MHz". */
export function channelText(station: WifiStation): string {
  if (station.channel === null) return ''
  return station.channelWidthMhz === null
    ? `canal ${station.channel}`
    : `canal ${station.channel} · ${station.channelWidthMhz} MHz`
}

/** Conectado há um dia, com MAC de fábrica: tem cara de câmera ou IoT. */
const FIXED_AFTER_S = 86_400

/** Sugestão para vigiar — só sugere, quem decide é o usuário. */
export function looksFixed(station: WifiStation): boolean {
  return !station.randomizedMac && (station.connectedS ?? 0) >= FIXED_AFTER_S
}

export const WATCH_STATE: Record<WifiWatchState, HealthPresentation> = {
  pending: { label: 'Aguardando leitura', color: 'info', icon: 'mdi-timer-sand' },
  connected: { label: 'Conectado', color: 'success', icon: 'mdi-wifi-check' },
  offline: { label: 'Fora do Wi-Fi', color: 'error', icon: 'mdi-wifi-off' },
}

/** Um atalho por linha: o que é cada coisa no texto da busca. */
function searchText(row: WifiStationRow): string {
  return [row.hostname, row.vendor, row.mac, row.ip, row.ssid, row.deviceName]
    .filter(Boolean)
    .join(' ')
    .toLowerCase()
}

export type WifiHealthFilter = 'all' | 'attention' | 'power_save'

export interface WifiStationFilter {
  health: WifiHealthFilter
  band: WifiBand | null
  deviceId: number | null
  /** Só os clientes desta rede (SSID). */
  ssid: string | null
  /** Só os aparelhos vigiados. */
  watchedOnly: boolean
  search: string
}

export function emptyFilter(): WifiStationFilter {
  return { health: 'all', band: null, deviceId: null, ssid: null, watchedOnly: false, search: '' }
}

/**
 * As linhas que passam no filtro. `watched` são os MACs vigiados (só pesa
 * com `watchedOnly`); o nome dado ao vigiado também entra na busca.
 */
export function filterStations(
  rows: WifiStationRow[],
  filter: WifiStationFilter,
  watched: ReadonlyMap<string, { name: string }> = new Map()
): WifiStationRow[] {
  const search = filter.search.trim().toLowerCase()
  return rows.filter((row) => {
    if (filter.health === 'attention' && !needsAttention(row.health)) return false
    if (filter.health === 'power_save' && row.health !== 'power_save') return false
    if (filter.band !== null && row.band !== filter.band) return false
    if (filter.deviceId !== null && row.deviceId !== filter.deviceId) return false
    if (filter.ssid !== null && row.ssid !== filter.ssid) return false
    if (filter.watchedOnly && !watched.has(row.mac)) return false
    if (!search) return true
    const name = watched.get(row.mac)?.name.toLowerCase() ?? ''
    return searchText(row).includes(search) || name.includes(search)
  })
}

/** As redes (SSID) vistas, em ordem alfabética, para o filtro. */
export function ssidsOf(rows: WifiStationRow[]): string[] {
  const names = new Set(rows.map((row) => row.ssid).filter((ssid): ssid is string => !!ssid))
  return [...names].sort((a, b) => a.localeCompare(b))
}

export interface WifiStationCounts {
  all: number
  attention: number
  powerSave: number
}

export function countStations(rows: WifiStationRow[]): WifiStationCounts {
  return {
    all: rows.length,
    attention: rows.filter((row) => needsAttention(row.health)).length,
    powerSave: rows.filter((row) => row.health === 'power_save').length,
  }
}

/**
 * As estações de vários equipamentos numa lista só: quem precisa de atenção
 * primeiro, depois o sinal mais fraco.
 */
export function stationRows(snapshots: WifiDeviceSnapshot[]): WifiStationRow[] {
  const rows = snapshots.flatMap((snapshot) =>
    snapshot.stations.map((station) => ({
      ...station,
      deviceId: snapshot.deviceId,
      deviceName: snapshot.deviceName,
      canPing: snapshot.canPing,
    }))
  )
  return rows.sort(
    (a, b) =>
      Number(needsAttention(b.health)) - Number(needsAttention(a.health)) ||
      a.signalDbm - b.signalDbm
  )
}

const READ_STATUSES: readonly WifiReadStatus[] = ['reading', 'ok', 'failed']

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

/** O payload de `wifi:snapshot` tem o formato esperado? */
export function isWifiDeviceSnapshot(value: unknown): value is WifiDeviceSnapshot {
  return (
    isRecord(value) &&
    typeof value.deviceId === 'number' &&
    typeof value.deviceName === 'string' &&
    READ_STATUSES.some((status) => status === value.status) &&
    Array.isArray(value.stations)
  )
}

/** O payload de `wifi:watched` tem o formato esperado? */
export function isWifiWatchedSnapshot(value: unknown): value is WifiWatchedSnapshot {
  return (
    isRecord(value) &&
    Array.isArray(value.clients) &&
    value.clients.every(
      (client) =>
        isRecord(client) && typeof client.id === 'number' && typeof client.mac === 'string'
    )
  )
}

/** O nome do ambiente da estimativa de distância (Configurações de Wi-Fi). */
export const ENVIRONMENT_LABEL: Record<WifiEnvironment, string> = {
  open: 'Aberto (galpão, área externa)',
  office: 'Escritório ou casa',
  walls: 'Muitas paredes (alvenaria, lajes)',
}

/**
 * A distância aproximada como o operador lê: a faixa ("~3–6 m") e, quando o
 * rádio não informou a potência, o aviso de que é menos confiável.
 */
export function distanceText(station: WifiStation): string {
  const distance = station.distance
  if (!distance) return 'Sem estimativa (o roteador não informou a banda)'
  const range = formatDistanceRange(distance.minM, distance.maxM)
  return distance.calibrated ? range : `${range} (estimativa sem a potência do rádio)`
}

/** Nome curto para o mapa: o do vigiado, o do DHCP ou o fim do MAC. */
export function shortStationName(station: WifiStation, watchedName?: string | null): string {
  if (watchedName) return watchedName
  if (station.hostname) return station.hostname
  return `…${station.mac.slice(-5)}`
}

/** Uma troca de AP numa frase: "AP Sala → AP Cozinha". */
export function roamRoute(event: WifiRoamEvent): string {
  return `${event.fromDeviceName} → ${event.toDeviceName}`
}

/** O payload de `wifi:roamed` tem o formato esperado? */
export function isWifiRoamEvent(value: unknown): value is WifiRoamEvent {
  return (
    isRecord(value) &&
    typeof value.id === 'number' &&
    typeof value.mac === 'string' &&
    typeof value.fromDeviceName === 'string' &&
    typeof value.toDeviceName === 'string' &&
    typeof value.occurredAt === 'string'
  )
}
