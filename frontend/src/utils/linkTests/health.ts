/**
 * CPU e memória das pontas durante o teste de velocidade (ADR 015 §12), lidos
 * a cada segundo no mesmo relógio da vazão (`atMs`). Funções puras: o gráfico
 * "vazão × CPU", a linha de saúde dos equipamentos no recorte do mapa e a
 * frase que diz se o processador limitou o teste.
 */
import type { LinkTest, LinkTestHealthSample, LinkTestSample } from '@/types/linkTests'
import { formatPercent } from '@/utils/formatters'
import { HEALTH_HIGH_PERCENT, nodeHealthText } from '@/utils/topology/presentation'

export interface ChartPoint {
  /** Milissegundos desde o início da medição da perna. */
  t: number
  v: number
}

/** Cores do CPU por equipamento, na ordem do caminho (contrastam com ida/volta). */
export const HEALTH_COLORS = ['#f59e0b', '#e879f9', '#fb7185', '#a3e635', '#facc15'] as const
/** Abaixo disso o processador não explica vazão baixa. */
export const CPU_RELAXED_PERCENT = 60

const byTime = (a: ChartPoint, b: ChartPoint) => a.t - b.t

/**
 * Vazão no relógio da perna. Perna gravada antes do `atMs`: a ida usa o
 * tempo do próprio sentido e a volta vem logo depois dela.
 */
export function trafficPoints(
  forward: readonly LinkTestSample[],
  reverse: readonly LinkTestSample[]
): { forward: ChartPoint[]; reverse: ChartPoint[] } {
  const timed = [...forward, ...reverse].every((sample) => typeof sample.atMs === 'number')
  if (timed) {
    const points = (list: readonly LinkTestSample[]) =>
      list.map((sample) => ({ t: sample.atMs ?? 0, v: sample.bps })).sort(byTime)
    return { forward: points(forward), reverse: points(reverse) }
  }
  const forwardPoints = forward.map((sample) => ({ t: sample.tMs, v: sample.bps })).sort(byTime)
  const offset = forwardPoints.length > 0 ? forwardPoints[forwardPoints.length - 1].t + 1000 : 0
  return {
    forward: forwardPoints,
    reverse: reverse.map((sample) => ({ t: offset + sample.tMs, v: sample.bps })).sort(byTime),
  }
}

export interface DeviceHealthSeries {
  deviceId: number
  name: string
  color: string
  cpu: ChartPoint[]
  memory: ChartPoint[]
  peakCpu: number | null
  lastMemory: number | null
}

/**
 * Uma série por equipamento lido, na ordem do caminho (`order`; os de fora
 * dela vão no fim). Cada um com a sua cor.
 */
export function healthSeries(
  samples: readonly LinkTestHealthSample[],
  names: Readonly<Record<number, string>>,
  order: readonly number[] = []
): DeviceHealthSeries[] {
  const ids = [...new Set(samples.map((sample) => sample.deviceId))].sort((a, b) => {
    const ia = order.indexOf(a)
    const ib = order.indexOf(b)
    return (ia < 0 ? Infinity : ia) - (ib < 0 ? Infinity : ib) || a - b
  })
  return ids.map((deviceId, index) => {
    const own = samples.filter((sample) => sample.deviceId === deviceId)
    const cpu = own
      .filter((sample) => typeof sample.cpu === 'number')
      .map((sample) => ({ t: sample.atMs, v: sample.cpu as number }))
      .sort(byTime)
    const memory = own
      .filter((sample) => typeof sample.memory === 'number')
      .map((sample) => ({ t: sample.atMs, v: sample.memory as number }))
      .sort(byTime)
    return {
      deviceId,
      name: names[deviceId] ?? `Dispositivo ${deviceId}`,
      color: HEALTH_COLORS[index % HEALTH_COLORS.length],
      cpu,
      memory,
      peakCpu: cpu.length > 0 ? Math.max(...cpu.map((point) => point.v)) : null,
      lastMemory: memory.length > 0 ? memory[memory.length - 1].v : null,
    }
  })
}

/** O valor mais perto de `t` (dentro de `tolerance` ms), ou `null`. */
export function valueAt(points: readonly ChartPoint[], t: number, tolerance = 1500): number | null {
  let best: ChartPoint | null = null
  for (const point of points) {
    if (!best || Math.abs(point.t - t) < Math.abs(best.t - t)) best = point
  }
  return best && Math.abs(best.t - t) <= tolerance ? best.v : null
}

/** Nome de cada equipamento do teste (pontas e os atravessados). */
export function testDeviceNames(test: LinkTest): Record<number, string> {
  const names: Record<number, string> = {}
  for (const leg of test.legs) {
    for (const end of [leg.a, leg.b]) if (end.deviceId !== null) names[end.deviceId] = end.name
    leg.viaDeviceIds.forEach((id, index) => {
      const name = leg.viaNames[index]
      if (name) names[id] = name
    })
  }
  return names
}

/**
 * A linha de saúde de cada equipamento no recorte do mapa: ao vivo, a última
 * leitura; depois do teste, o pico de CPU ("CPU máx 97%") e a memória.
 */
export function testHealthLabels(
  samples: readonly LinkTestHealthSample[],
  active: boolean
): Record<number, string> {
  const labels: Record<number, string> = {}
  for (const series of healthSeries(samples, {})) {
    const cpu = active ? (series.cpu[series.cpu.length - 1]?.v ?? null) : series.peakCpu
    const text = nodeHealthText(
      { cpuPercent: cpu, memoryPercent: series.lastMemory },
      { peak: !active }
    )
    if (text) labels[series.deviceId] = text
  }
  return labels
}

/**
 * O processador explica o resultado? "CPU do AP-Terraço chegou a 97%…" com
 * algum equipamento no teto; "CPU ficou abaixo de 60%…" com todos folgados;
 * `null` sem leitura ou no meio-termo.
 */
export function cpuVerdict(series: readonly DeviceHealthSeries[]): string | null {
  const read = series.filter((item) => item.peakCpu !== null)
  if (read.length === 0) return null
  const busiest = read.reduce((a, b) => ((b.peakCpu ?? 0) > (a.peakCpu ?? 0) ? b : a))
  const peak = busiest.peakCpu ?? 0
  if (peak >= HEALTH_HIGH_PERCENT) {
    return `CPU do ${busiest.name} chegou a ${formatPercent(peak, 0)} durante a medição — o processador é o provável limite.`
  }
  if (peak < CPU_RELAXED_PERCENT) {
    const who = read.length === 1 ? `do ${busiest.name}` : 'dos equipamentos'
    return `CPU ${who} ficou em no máximo ${formatPercent(peak, 0)} — não é o processador que limita.`
  }
  return null
}
