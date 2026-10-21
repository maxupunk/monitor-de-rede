/**
 * CPU e memória das pontas no teste de velocidade (ADR 015 §12): lidos a cada
 * segundo desde o começo da perna (o "antes", sem tráfego) até alguns segundos
 * depois da medição, no mesmo relógio da vazão (`atMs`) e com a fase marcada.
 * Funções puras: a linha do tempo do gráfico, o "antes → no teste → depois" de
 * cada equipamento, o rótulo no recorte do mapa e a frase do resumo.
 */
import type { LinkTest, LinkTestHealthSample, LinkTestSample } from '@/types/linkTests'
import { formatElapsedMs, formatPercent } from '@/utils/formatters'
import { HEALTH_HIGH_PERCENT, nodeHealthText } from '@/utils/topology/presentation'

export interface ChartPoint {
  /** Milissegundos no relógio da perna. */
  t: number
  v: number
}

/** Abaixo disso, no pico, o processador não explica vazão baixa. */
export const CPU_RELAXED_PERCENT = 60
/** "Já estava ocupado": o antes a partir disso. */
export const CPU_BUSY_BEFORE_PERCENT = 80
/** Quanto do "antes" o gráfico mostra. */
export const BEFORE_SHOWN_MS = 8000
/** Diferença de memória que vale contar ("RAM 74% → 86%"). */
const MEMORY_CHANGE_POINTS = 3

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

/** Uso num momento: CPU e memória (%), qualquer um pode faltar. */
export interface Usage {
  cpu: number | null
  memory: number | null
}

export interface DeviceHealth {
  deviceId: number
  name: string
  cpu: ChartPoint[]
  memory: ChartPoint[]
  /** A última leitura antes do tráfego. */
  before: Usage
  /** O máximo com o tráfego passando. */
  peak: Usage
  /** A última leitura depois da medição. */
  after: Usage
}

function last<T>(list: readonly T[]): T | undefined {
  return list[list.length - 1]
}

function usageOf(samples: readonly LinkTestHealthSample[]): Usage {
  const cpu = last(samples.filter((sample) => sample.cpu !== null))?.cpu ?? null
  const memory = last(samples.filter((sample) => sample.memory !== null))?.memory ?? null
  return { cpu, memory }
}

function maxOf(values: (number | null)[]): number | null {
  const present = values.filter((value): value is number => value !== null)
  return present.length > 0 ? Math.max(...present) : null
}

/**
 * Um equipamento por item, na ordem do caminho (`order`; os de fora dela vão
 * no fim), com o antes, o pico no teste e o depois.
 */
export function deviceHealth(
  samples: readonly LinkTestHealthSample[],
  names: Readonly<Record<number, string>>,
  order: readonly number[] = []
): DeviceHealth[] {
  const ids = [...new Set(samples.map((sample) => sample.deviceId))].sort((a, b) => {
    const ia = order.indexOf(a)
    const ib = order.indexOf(b)
    return (ia < 0 ? Infinity : ia) - (ib < 0 ? Infinity : ib) || a - b
  })
  return ids.map((deviceId) => {
    const own = samples
      .filter((sample) => sample.deviceId === deviceId)
      .sort((a, b) => a.atMs - b.atMs)
    const during = own.filter((sample) => sample.phase === 'during')
    const points = (pick: (sample: LinkTestHealthSample) => number | null) =>
      own.flatMap((sample) => {
        const v = pick(sample)
        return v === null ? [] : [{ t: sample.atMs, v }]
      })
    return {
      deviceId,
      name: names[deviceId] ?? `Dispositivo ${deviceId}`,
      cpu: points((sample) => sample.cpu),
      memory: points((sample) => sample.memory),
      before: usageOf(own.filter((sample) => sample.phase === 'before')),
      peak: {
        cpu: maxOf(during.map((sample) => sample.cpu)),
        memory: maxOf(during.map((sample) => sample.memory)),
      },
      after: usageOf(own.filter((sample) => sample.phase === 'after')),
    }
  })
}

/** Um trecho de tempo com o valor medido nele. */
export interface MeasuredStep {
  from: number
  to: number
  v: number
}

/**
 * Leituras de contador viram trechos: um valor descreve o intervalo desde a
 * mudança anterior até ele (o net-snmp renova o contador a cada ~5 s e a
 * leitura repete o último valor enquanto isso). Assim o degrau cai no tempo
 * que ele mediu, junto do tráfego que o causou, e não depois dele.
 */
export function measuredSteps(points: readonly ChartPoint[]): MeasuredStep[] {
  if (points.length === 0) return []
  const changes: ChartPoint[] = [points[0]]
  for (const point of points.slice(1)) {
    if (point.v !== changes[changes.length - 1].v) changes.push(point)
  }
  const steps: MeasuredStep[] = []
  for (let i = 1; i < changes.length; i++) {
    steps.push({ from: changes[i - 1].t, to: changes[i].t, v: changes[i].v })
  }
  const lastChange = changes[changes.length - 1]
  const end = points[points.length - 1].t
  if (steps.length === 0 || end > lastChange.t) {
    steps.push({ from: lastChange.t, to: Math.max(end, lastChange.t), v: lastChange.v })
  }
  return steps
}

/** O valor medido no instante `t` (o trecho que o contém), ou `null` fora deles. */
export function stepValueAt(steps: readonly MeasuredStep[], t: number): number | null {
  return steps.find((step) => t >= step.from && t <= step.to)?.v ?? null
}

/** O valor mais perto de `t` (dentro de `tolerance` ms), ou `null`. */
export function valueAt(points: readonly ChartPoint[], t: number, tolerance = 1500): number | null {
  let best: ChartPoint | null = null
  for (const point of points) {
    if (!best || Math.abs(point.t - t) < Math.abs(best.t - t)) best = point
  }
  return best && Math.abs(best.t - t) <= tolerance ? best.v : null
}

const pct = (value: number) => formatPercent(value, 0)

/** "10% → 96%" (ou só o que há). */
function change(before: number | null, peak: number | null): string | null {
  if (before !== null && peak !== null) return `${pct(before)} → ${pct(peak)}`
  if (peak !== null) return pct(peak)
  return before !== null ? pct(before) : null
}

/**
 * O resumo de um equipamento: "CPU 10% → 96% no teste, 12% depois · RAM 74%"
 * — a memória só mostra a mudança quando ela anda de verdade.
 */
export function usageStory(device: DeviceHealth): string | null {
  const parts: string[] = []
  const cpu = change(device.before.cpu, device.peak.cpu)
  if (cpu) {
    const after = device.after.cpu !== null ? `, ${pct(device.after.cpu)} depois` : ''
    parts.push(`CPU ${cpu}${device.peak.cpu !== null ? ' no teste' : ' antes'}${after}`)
  }
  const memoryBefore = device.before.memory
  const memoryTest = device.peak.memory ?? device.after.memory
  if (memoryBefore !== null || memoryTest !== null) {
    const moved =
      memoryBefore !== null &&
      memoryTest !== null &&
      Math.abs(memoryTest - memoryBefore) >= MEMORY_CHANGE_POINTS
    parts.push(
      `RAM ${moved ? change(memoryBefore, memoryTest) : pct(memoryTest ?? memoryBefore ?? 0)}`
    )
  }
  return parts.length > 0 ? parts.join(' · ') : null
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
 * A linha de saúde de cada equipamento no recorte do mapa. Ao vivo, a
 * leitura mais recente (de qualquer perna — o rótulo não some entre uma perna e
 * outra); no fim, o antes e o pico: "CPU 10% → 96% · RAM 86%".
 */
export function testHealthLabels(
  samples: readonly LinkTestHealthSample[],
  active: boolean
): Record<number, string> {
  const labels: Record<number, string> = {}
  for (const device of deviceHealth(samples, {})) {
    let text: string | null
    if (active) {
      text = nodeHealthText({
        cpuPercent: last(device.cpu)?.v ?? null,
        memoryPercent: last(device.memory)?.v ?? null,
      })
    } else {
      // Curto, para caber sob o nome: "CPU 11→96%".
      const { cpu: before, memory: memoryBefore } = device.before
      const peak = device.peak.cpu
      const cpu =
        before !== null && peak !== null
          ? `CPU ${Math.round(before)}→${pct(peak)}`
          : (peak ?? before) !== null
            ? `CPU ${pct((peak ?? before) as number)}`
            : null
      const memory = device.peak.memory ?? device.after.memory ?? memoryBefore
      const parts = [cpu, memory !== null ? `RAM ${pct(memory)}` : null]
      const high = (device.peak.cpu ?? 0) >= HEALTH_HIGH_PERCENT
      text = parts.some(Boolean) ? `${high ? '⚠ ' : ''}${parts.filter(Boolean).join(' · ')}` : null
    }
    if (text) labels[device.deviceId] = text
  }
  return labels
}

/**
 * O processador explica o resultado? Olha o equipamento de maior pico e diz,
 * comparando com o antes, se foi o teste que o levou lá ou se ele já estava
 * ocupado. `null` sem leitura no teste.
 */
export function cpuVerdict(devices: readonly DeviceHealth[]): string | null {
  const read = devices.filter((device) => device.peak.cpu !== null)
  if (read.length === 0) return null
  const busiest = read.reduce((a, b) => ((b.peak.cpu ?? 0) > (a.peak.cpu ?? 0) ? b : a))
  const peak = busiest.peak.cpu ?? 0
  const before = busiest.before.cpu
  const after = busiest.after.cpu
  const name = busiest.name
  if (peak >= HEALTH_HIGH_PERCENT) {
    if (before !== null && before >= CPU_BUSY_BEFORE_PERCENT) {
      return `O ${name} já estava com a CPU em ${pct(before)} antes do teste e chegou a ${pct(peak)} — ele já estava ocupado antes de medir.`
    }
    const rise =
      before !== null ? `subiu de ${pct(before)} para ${pct(peak)}` : `chegou a ${pct(peak)}`
    const back = after !== null ? ` (voltou a ${pct(after)} depois)` : ''
    return `A CPU do ${name} ${rise} no teste${back} — o processador é o provável limite.`
  }
  if (peak < CPU_RELAXED_PERCENT) {
    const what = before !== null ? `de ${pct(before)} para ${pct(peak)}` : `até ${pct(peak)}`
    return `A CPU ficou folgada (${name}: ${what} no teste) — não é o processador que limita.`
  }
  const rise =
    before !== null ? `subiu de ${pct(before)} para ${pct(peak)}` : `chegou a ${pct(peak)}`
  return `A CPU do ${name} ${rise} no teste.`
}

/** "−8 s", "0 s", "5,5 s": o tempo em relação ao começo da medição. */
export function relativeTime(ms: number): string {
  if (Math.abs(ms) < 50) return '0 s'
  return ms < 0 ? `−${formatElapsedMs(-ms)}` : formatElapsedMs(ms)
}

/** Onde o tráfego começa e termina no relógio da perna (`null` sem vazão). */
export function testWindow(
  forward: readonly ChartPoint[],
  reverse: readonly ChartPoint[],
  health: readonly LinkTestHealthSample[]
): { start: number; end: number } | null {
  const traffic = [...forward, ...reverse].map((point) => point.t)
  const during = health.filter((sample) => sample.phase === 'during').map((sample) => sample.atMs)
  if (traffic.length === 0 && during.length === 0) return null
  // A primeira amostra de vazão fecha o primeiro segundo: o tráfego começou ~1 s antes.
  const start = Math.min(...traffic.map((t) => t - 1000), ...during)
  const end = Math.max(...traffic, ...during)
  return { start: Math.max(0, start), end }
}
