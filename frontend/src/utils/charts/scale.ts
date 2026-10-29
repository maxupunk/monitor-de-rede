/**
 * Núcleo de geometria dos gráficos: escala, marcas do eixo e caminhos SVG.
 *
 * Tudo aqui trabalha em **pixels reais** do elemento medido. Os gráficos
 * antigos desenhavam numa caixa fixa (`viewBox="0 0 800 240"`) esticada com
 * `preserveAspectRatio="none"`: a escala horizontal ficava diferente da
 * vertical e o traço engrossava ou afinava conforme a inclinação da linha,
 * os pontos viravam elipses e o texto saía achatado. Desenhar na medida real
 * acaba com as três coisas de uma vez.
 */

/** Espessuras únicas do sistema — nenhum gráfico escolhe a sua. */
export const CHART_STROKE = {
  line: 1.5,
  sparkline: 1.25,
  grid: 1,
  crosshair: 1,
} as const

/**
 * Paleta categórica (passos validados para o tema escuro): azul, laranja,
 * água, amarelo, magenta, verde e violeta. Ordem fixa — a cor de uma série é
 * a da sua posição, nunca sorteada.
 */
export const CHART_PALETTE = [
  '#3987e5',
  '#d95926',
  '#199e70',
  '#c98500',
  '#d55181',
  '#008300',
  '#9085e9',
] as const

/** Cores semânticas dos gráficos (estado, não categoria). */
export const CHART_STATUS_COLORS = {
  ok: '#199e70',
  warning: '#c98500',
  critical: '#e5484d',
  info: '#3987e5',
  muted: '#8b949e',
} as const

/** Cor da série na posição `index` da paleta. */
export function paletteColor(index: number): string {
  return CHART_PALETTE[
    ((index % CHART_PALETTE.length) + CHART_PALETTE.length) % CHART_PALETTE.length
  ]
}

/**
 * Teto "redondo" do eixo: 1, 2, 2,5 ou 5 × baseⁿ acima do pico. Bytes usam
 * base 1024, para os rótulos saírem em GiB inteiros e não em 9,31 GiB.
 */
export function niceCeiling(value: number, base: 10 | 1024 = 10): number {
  if (!Number.isFinite(value) || value <= 0) return 1
  if (base === 1024) {
    // Dentro da unidade binária (KiB, MiB, GiB…) o arredondamento é decimal.
    const unit = 1024 ** Math.max(0, Math.floor(Math.log(value) / Math.log(1024)))
    return niceCeiling(value / unit) * unit
  }
  const magnitude = 10 ** Math.floor(Math.log10(value))
  const step = [1, 2, 2.5, 5, 10].find((factor) => factor * magnitude >= value) ?? 10
  return step * magnitude
}

export interface DomainOptions {
  /** Teto mínimo (ex.: 100 para percentuais). */
  minCeiling?: number
  /** Base do arredondamento do teto: 1024 para bytes. */
  base?: 10 | 1024
  /** Domínio fixo (estado binário, percentuais travados). */
  fixed?: { min: number; max: number }
}

/** Domínio vertical: começa em zero (ou no menor negativo) e sobe até um teto redondo. */
export function yDomain(
  values: number[],
  options: DomainOptions = {}
): { min: number; max: number } {
  if (options.fixed) return { ...options.fixed }
  const finite = values.filter((v) => Number.isFinite(v))
  const peak = finite.length > 0 ? Math.max(...finite) : 0
  const floor = finite.length > 0 ? Math.min(0, ...finite) : 0
  const ceiling = niceCeiling(Math.max(peak, options.minCeiling ?? 0), options.base ?? 10)
  return { min: floor < 0 ? -niceCeiling(-floor, options.base ?? 10) : 0, max: ceiling }
}

/** `count + 1` marcas igualmente espaçadas entre `min` e `max` (inclusive). */
export function axisTicks(min: number, max: number, count = 4): number[] {
  if (count <= 0 || !(max > min)) return [min]
  const step = (max - min) / count
  return Array.from({ length: count + 1 }, (_, i) => min + step * i)
}

export interface PlotArea {
  left: number
  top: number
  width: number
  height: number
}

/** Posição horizontal da amostra `index` de `count` (um ponto só fica no meio). */
export function xAt(index: number, count: number, area: PlotArea): number {
  if (count <= 1) return area.left + area.width / 2
  return area.left + (index / (count - 1)) * area.width
}

/** Posição vertical de `value` no domínio, presa à área do gráfico. */
export function yAt(value: number, domain: { min: number; max: number }, area: PlotArea): number {
  const range = domain.max - domain.min
  const ratio = range > 0 ? (value - domain.min) / range : 0
  const clamped = Math.min(1, Math.max(0, ratio))
  return area.top + area.height - clamped * area.height
}

/** Amostra mais próxima da coordenada `x` (para crosshair e tooltip). */
export function nearestIndex(x: number, count: number, area: PlotArea): number {
  if (count <= 1) return 0
  const ratio = (x - area.left) / area.width
  return Math.min(count - 1, Math.max(0, Math.round(ratio * (count - 1))))
}

export interface PlotPoint {
  x: number
  /** `null` = falta de amostra: a linha quebra em vez de inventar um valor. */
  y: number | null
}

const fmt = (n: number) => (Math.round(n * 10) / 10).toString()

/**
 * Caminho SVG de uma linha. Trechos sem amostra (`y === null`) quebram o
 * traço; `step` desenha degraus (estado binário, contadores).
 */
export function linePath(points: PlotPoint[], curve: 'linear' | 'step' = 'linear'): string {
  let d = ''
  let previous: { x: number; y: number } | null = null
  for (const point of points) {
    if (point.y === null) {
      previous = null
      continue
    }
    if (!previous) {
      d += `M${fmt(point.x)} ${fmt(point.y)}`
    } else if (curve === 'step') {
      d += `H${fmt(point.x)}V${fmt(point.y)}`
    } else {
      d += `L${fmt(point.x)} ${fmt(point.y)}`
    }
    previous = { x: point.x, y: point.y }
  }
  return d
}

/** Área sob a linha até `baseY`, um polígono por trecho contínuo. */
export function areaPath(
  points: PlotPoint[],
  baseY: number,
  curve: 'linear' | 'step' = 'linear'
): string {
  const segments: { x: number; y: number }[][] = []
  let current: { x: number; y: number }[] = []
  for (const point of points) {
    if (point.y === null) {
      if (current.length > 0) segments.push(current)
      current = []
    } else {
      current.push({ x: point.x, y: point.y })
    }
  }
  if (current.length > 0) segments.push(current)

  return segments
    .filter((segment) => segment.length > 1)
    .map((segment) => {
      const first = segment[0]
      const last = segment[segment.length - 1]
      // A linha começa com "M x y": vira "L x y" depois de descer à base.
      const line = linePath(segment, curve).slice(1)
      return `M${fmt(first.x)} ${fmt(baseY)}L${line}L${fmt(last.x)} ${fmt(baseY)}Z`
    })
    .join('')
}
