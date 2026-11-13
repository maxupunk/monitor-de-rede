/**
 * Desenho radial do mapa Wi-Fi, puro (testável sem DOM): cada AP é um centro
 * e os aparelhos dele ficam em volta, à distância estimada pelo sinal.
 *
 * - **Escala logarítmica com teto:** 1 m e 3 m ficam distinguíveis perto do
 *   AP, e quem está a 80 m não some da tela — tudo além de {@link CAP_METERS}
 *   fica na borda.
 * - **Ordem estável:** os aparelhos repartem a volta por igual, na ordem do
 *   hash do MAC — quem entra ou sai não embaralha os outros. Quando aperta,
 *   alternam de anel para não virar borrão.
 * - **APs lado a lado** numa grade, cada um na sua célula, sem sobreposição.
 */
import type { WifiReadStatus } from '@/bindings/WifiReadStatus'
import type { WifiStationRow } from '@/utils/wifiPresentation'

/** O raio do AP (ícone e rótulo) — ninguém fica dentro dele. */
export const AP_CORE = 52
/** A borda do desenho: aparelhos além do teto ficam aqui. */
export const MAX_RADIUS = 250
/** Além disto, a escala não diferencia (fica na borda). */
export const CAP_METERS = 60
/** Os anéis de referência da escala, em metros. */
export const RING_METERS: readonly number[] = [5, 10, 20, 40]
/** Distância mínima, em px, entre dois aparelhos vizinhos no mesmo anel. */
const MIN_ARC = 64
/** O deslocamento radial de quem alterna de anel quando está apertado. */
const STAGGER = 24
/** Folga entre as células de dois APs. */
const CELL_GAP = 90
/** Sem estimativa de distância, o aparelho fica neste ponto da escala. */
const UNKNOWN_FRACTION = 0.7

export interface MapAp {
  deviceId: number
  deviceName: string
  status: WifiReadStatus
  error: string | null
}

export interface Point {
  x: number
  y: number
}

export interface PlacedAp extends Point {
  ap: MapAp
}

export interface PlacedStation extends Point {
  row: WifiStationRow
  /** Distância do centro do AP no desenho (px). */
  radius: number
}

export interface PlacedRing extends Point {
  deviceId: number
  meters: number
  radius: number
}

export interface WifiMapLayout {
  aps: PlacedAp[]
  stations: PlacedStation[]
  rings: PlacedRing[]
}

/** O raio do desenho para uma distância (px), com teto. */
export function radiusFor(meters: number | null | undefined): number {
  if (meters === null || meters === undefined || !Number.isFinite(meters)) {
    return AP_CORE + (MAX_RADIUS - AP_CORE) * UNKNOWN_FRACTION
  }
  const clamped = Math.min(Math.max(meters, 0), CAP_METERS)
  return AP_CORE + ((MAX_RADIUS - AP_CORE) * Math.log1p(clamped)) / Math.log1p(CAP_METERS)
}

/** Um número estável em [0, 1) a partir do texto (FNV-1a). */
export function seedOf(text: string): number {
  let hash = 0x811c9dc5
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return (hash >>> 0) / 0x1_0000_0000
}

/** Folga de atividade que separa a associação nova da sobra da antiga. */
const STALE_MARGIN_MS = 10_000

/**
 * Um aparelho, um ponto: visto por dois APs no meio de um roaming, vale o de
 * atividade mais recente e, empatados, o de sinal melhor (o mesmo critério do
 * backend, `roaming::detect::locate`).
 */
export function dedupeByMac(rows: WifiStationRow[]): WifiStationRow[] {
  const best = new Map<string, WifiStationRow>()
  for (const row of rows) {
    const current = best.get(row.mac)
    if (!current || beats(row, current)) best.set(row.mac, row)
  }
  return [...best.values()]
}

function beats(candidate: WifiStationRow, current: WifiStationRow): boolean {
  const mine = candidate.inactiveMs ?? 0
  const theirs = current.inactiveMs ?? 0
  if (mine + STALE_MARGIN_MS < theirs) return true
  if (theirs + STALE_MARGIN_MS < mine) return false
  return candidate.signalDbm > current.signalDbm
}

/**
 * Os aparelhos repartem a volta por igual, na ordem do hash do MAC: a maior
 * folga possível entre vizinhos (rótulo não encosta em rótulo) e a ordem em
 * volta do AP não muda — quem entra só abre espaço entre dois vizinhos, e o
 * mapa anima o ajuste.
 */
function evenAngles(count: number): number[] {
  return Array.from({ length: count }, (_, index) => (Math.PI * 2 * index) / count)
}

/** A célula de cada AP: lado e posição do centro na grade. */
function cellCenter(index: number, columns: number): Point {
  const cell = (MAX_RADIUS + STAGGER) * 2 + CELL_GAP
  return { x: (index % columns) * cell, y: Math.floor(index / columns) * cell }
}

/**
 * O desenho inteiro: APs em grade (por nome) e os aparelhos de cada um em
 * volta. `rows` já filtradas e sem MAC repetido ({@link dedupeByMac}).
 */
export function layoutWifiMap(aps: MapAp[], rows: WifiStationRow[]): WifiMapLayout {
  const ordered = [...aps].sort((a, b) => a.deviceName.localeCompare(b.deviceName))
  const columns = Math.max(1, Math.ceil(Math.sqrt(ordered.length)))
  const layout: WifiMapLayout = { aps: [], stations: [], rings: [] }

  ordered.forEach((ap, index) => {
    const center = cellCenter(index, columns)
    layout.aps.push({ ap, ...center })
    for (const meters of RING_METERS) {
      layout.rings.push({ deviceId: ap.deviceId, meters, radius: radiusFor(meters), ...center })
    }

    const mine = rows
      .filter((row) => row.deviceId === ap.deviceId)
      .map((row) => ({ row, seed: seedOf(row.mac), radius: radiusFor(row.distance?.meters) }))
      .sort((a, b) => a.seed - b.seed || a.row.mac.localeCompare(b.row.mac))
    if (mine.length === 0) return

    const medianRadius = [...mine].sort((a, b) => a.radius - b.radius)[Math.floor(mine.length / 2)]
      .radius
    const angles = evenAngles(mine.length)
    // Mais aparelhos do que cabem num anel: alterna para fora.
    const crowded = mine.length * MIN_ARC > Math.PI * 2 * medianRadius
    mine.forEach((item, position) => {
      const radius = crowded ? item.radius + (position % 3) * STAGGER : item.radius
      const angle = angles[position] - Math.PI / 2
      layout.stations.push({
        row: item.row,
        radius,
        x: center.x + Math.cos(angle) * radius,
        y: center.y + Math.sin(angle) * radius,
      })
    })
  })
  return layout
}
