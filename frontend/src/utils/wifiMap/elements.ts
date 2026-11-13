/**
 * Do desenho ({@link layoutWifiMap}) aos elementos do Cytoscape, puro: tudo o
 * que muda a aparência vira `data` (rótulo, cor, ícone) ou `classes`
 * (vigiado, em repouso, precisa de atenção). O mapa só compara e aplica a
 * diferença (`syncElements`), sem redesenhar a cada snapshot.
 */
import { mdiAccessPoint } from '@mdi/js'
import type { GraphElement } from '@/utils/topology/elementSync'
import { iconDataUri, type TopologyPalette } from '@/utils/topology/style'
import { formatDistanceRange } from '@/utils/formatters'
import { needsAttention, shortStationName } from '@/utils/wifiPresentation'
import { DEVICE_KIND, deviceKind } from './deviceKind'
import type { WifiMapLayout } from './layout'
import { linkColor, linkWidth } from './style'

export const apNodeId = (deviceId: number) => `ap-${deviceId}`
export const stationNodeId = (mac: string) => `st-${mac}`
/** A linha leva o AP no id: na troca de AP ela é outra (a antiga sai, a nova entra). */
export const linkEdgeId = (deviceId: number, mac: string) => `ln-${deviceId}-${mac}`
const ringNodeId = (deviceId: number, meters: number) => `ring-${deviceId}-${meters}`

/** O MAC de um id de aparelho ou de linha do mapa; `null` para AP e anel. */
export function macOfElement(id: string): string | null {
  if (id.startsWith('st-')) return id.slice(3)
  if (id.startsWith('ln-')) return id.slice(id.indexOf('-', 3) + 1)
  return null
}

export interface WifiMapElementsInput {
  layout: WifiMapLayout
  palette: TopologyPalette
  /** Os vigiados pelo MAC: nome e anel. */
  watched: ReadonlyMap<string, { name: string }>
  selectedMac: string | null
}

const AP_STATUS_LABEL = { ok: '', reading: ' · lendo…', failed: ' · leitura falhou' } as const

function stationLabel(name: string, signalDbm: number): string {
  return `${name}\n${signalDbm} dBm`
}

export function buildWifiMapElements(input: WifiMapElementsInput): GraphElement[] {
  const { layout, palette, watched, selectedMac } = input
  const iconColor = palette.onSurface
  const elements: GraphElement[] = []

  for (const ring of layout.rings) {
    elements.push({
      group: 'nodes',
      data: {
        id: ringNodeId(ring.deviceId, ring.meters),
        label: `${ring.meters} m`,
        size: Math.round(ring.radius * 2),
      },
      classes: 'ring',
      position: { x: ring.x, y: ring.y },
    })
  }

  const counts = new Map<number, number>()
  for (const station of layout.stations) {
    counts.set(station.row.deviceId, (counts.get(station.row.deviceId) ?? 0) + 1)
  }

  for (const placed of layout.aps) {
    const { ap } = placed
    const count = counts.get(ap.deviceId) ?? 0
    elements.push({
      group: 'nodes',
      data: {
        id: apNodeId(ap.deviceId),
        deviceId: ap.deviceId,
        label: `${ap.deviceName}\n${count === 1 ? '1 aparelho' : `${count} aparelhos`}${AP_STATUS_LABEL[ap.status]}`,
        icon: iconDataUri(mdiAccessPoint, '#FFFFFF'),
      },
      classes: `ap ap-${ap.status}`,
      position: { x: placed.x, y: placed.y },
    })
  }

  for (const placed of layout.stations) {
    const { row } = placed
    const watchedName = watched.get(row.mac)?.name ?? null
    const color = linkColor(row.linkScore, palette)
    const kind = deviceKind(row)
    const classes = ['station']
    if (watchedName) classes.push('watched')
    if (row.health === 'power_save') classes.push('sleeping')
    if (needsAttention(row.health)) classes.push('attention')
    if (row.mac === selectedMac) classes.push('selected')
    elements.push({
      group: 'nodes',
      data: {
        id: stationNodeId(row.mac),
        mac: row.mac,
        apId: row.deviceId,
        label: stationLabel(shortStationName(row, watchedName), row.signalDbm),
        icon: iconDataUri(DEVICE_KIND[kind].path, iconColor),
        color,
      },
      classes: classes.join(' '),
      position: { x: placed.x, y: placed.y },
    })
    const linkClasses = ['link']
    if (row.health === 'power_save') linkClasses.push('sleeping')
    if (row.mac === selectedMac) linkClasses.push('focus')
    elements.push({
      group: 'edges',
      data: {
        id: linkEdgeId(row.deviceId, row.mac),
        source: apNodeId(row.deviceId),
        target: stationNodeId(row.mac),
        color,
        width: linkWidth(row.txRateBps),
        distanceLabel: row.distance
          ? formatDistanceRange(row.distance.minM, row.distance.maxM)
          : 'distância desconhecida',
      },
      classes: linkClasses.join(' '),
    })
  }
  return elements
}
