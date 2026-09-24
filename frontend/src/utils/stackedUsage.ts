/**
 * Regras puras do gráfico de consumo empilhado (CPU/RAM por container): quais
 * containers ganham faixa própria, a cor de cada um e onde cada faixa começa
 * e termina em cada amostra.
 */

/** Uma amostra: o instante e o valor de cada série (id → valor). */
export interface UsageSample {
  time: string
  values: Record<string, number>
}

export interface StackedLayer {
  id: string
  label: string
  color: string
  /** Valor da série em cada amostra (0 quando ausente). */
  values: number[]
  /** Base e topo da faixa em cada amostra. */
  lower: number[]
  upper: number[]
}

export interface StackedUsage {
  layers: StackedLayer[]
  totals: number[]
  times: string[]
  /** Maior total da janela: o teto do eixo. */
  peak: number
}

/**
 * Paleta categórica (passos validados para o tema escuro): azul, laranja,
 * água, amarelo, magenta, verde e violeta. Ordem fixa, nunca ciclada — o que
 * passa de 7 séries vira "Outros".
 */
export const STACK_PALETTE = [
  '#3987e5',
  '#d95926',
  '#199e70',
  '#c98500',
  '#d55181',
  '#008300',
  '#9085e9',
] as const

/** Cor do agrupamento "Outros": neutra, para não competir com as séries. */
export const OTHERS_COLOR = '#757575'
export const OTHERS_ID = '__outros__'

/**
 * Empilha as séries. As `maxSeries` de maior média na janela ganham faixa
 * própria; o resto se soma em "Outros". A cor segue a série pela ordem em que
 * ela apareceu na janela, não pelo ranking — um container que sobe de posição
 * não troca de cor a cada amostra.
 */
export function buildStackedUsage(
  samples: UsageSample[],
  labels: Record<string, string>,
  maxSeries: number = STACK_PALETTE.length
): StackedUsage {
  const times = samples.map((sample) => sample.time)
  const firstSeen: string[] = []
  const sums = new Map<string, number>()
  for (const sample of samples) {
    const ids = Object.keys(sample.values).sort((a, b) =>
      (labels[a] ?? a).localeCompare(labels[b] ?? b)
    )
    for (const id of ids) {
      if (!sums.has(id)) firstSeen.push(id)
      sums.set(id, (sums.get(id) ?? 0) + Math.max(0, sample.values[id] ?? 0))
    }
  }

  const limit = Math.min(Math.max(0, maxSeries), STACK_PALETTE.length)
  const ranked = [...sums.entries()].sort((a, b) => b[1] - a[1]).map(([id]) => id)
  const own = new Set(ranked.slice(0, limit))
  const shown = firstSeen.filter((id) => own.has(id))
  const rest = firstSeen.filter((id) => !own.has(id))

  const valueOf = (sample: UsageSample, id: string) => Math.max(0, sample.values[id] ?? 0)
  const definitions: Array<{ id: string; label: string; color: string; ids: string[] }> = shown.map(
    (id, index) => ({
      id,
      label: labels[id] ?? id,
      color: STACK_PALETTE[index],
      ids: [id],
    })
  )
  if (rest.length > 0) {
    definitions.push({
      id: OTHERS_ID,
      // Sobrou um só: o nome dele diz mais que "Outros (1)".
      label: rest.length === 1 ? (labels[rest[0]] ?? rest[0]) : `Outros (${rest.length})`,
      color: OTHERS_COLOR,
      ids: rest,
    })
  }

  const base = samples.map(() => 0)
  const layers: StackedLayer[] = definitions.map((definition) => {
    const values = samples.map((sample) =>
      definition.ids.reduce((total, id) => total + valueOf(sample, id), 0)
    )
    const lower = [...base]
    const upper = values.map((value, index) => lower[index] + value)
    upper.forEach((value, index) => (base[index] = value))
    return {
      id: definition.id,
      label: definition.label,
      color: definition.color,
      values,
      lower,
      upper,
    }
  })

  return { layers, totals: base, times, peak: Math.max(0, ...base) }
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

/** Máximo de séries no tooltip: acima disso vira "+N outros". */
export const MAX_TOOLTIP_ROWS = 3

export interface StackPick {
  /** Faixa sob o cursor; `null` acima da pilha ou fora dela. */
  hovered: string | null
  /** Ids a mostrar, de cima para baixo como no gráfico. */
  rows: string[]
  /** Séries com valor no instante que ficaram de fora. */
  hidden: number
}

/**
 * Quais faixas o tooltip mostra no instante `index`, com o cursor na altura
 * `value` (na unidade do eixo).
 *
 * Sobre uma faixa, só ela. Se ela é fina demais para mirar (`minBand`, o
 * equivalente a alguns pixels), entram as vizinhas cujo centro está perto do
 * cursor — até [`MAX_TOOLTIP_ROWS`]. Acima da pilha, os maiores consumidores.
 */
export function pickLayers(
  stack: StackedUsage,
  index: number,
  value: number,
  minBand: number
): StackPick {
  const present = stack.layers.filter((layer) => (layer.values[index] ?? 0) > 0)
  const topDown = (ids: string[]) =>
    present
      .filter((layer) => ids.includes(layer.id))
      .reverse()
      .map((layer) => layer.id)
  const pick = (hovered: string | null, ids: string[]): StackPick => ({
    hovered,
    rows: topDown(ids),
    hidden: present.length - ids.length,
  })

  const hovered = present.find((layer) => value >= layer.lower[index] && value < layer.upper[index])
  if (!hovered) {
    const largest = [...present]
      .sort((a, b) => (b.values[index] ?? 0) - (a.values[index] ?? 0))
      .slice(0, MAX_TOOLTIP_ROWS)
      .map((layer) => layer.id)
    return pick(null, largest)
  }
  if ((hovered.values[index] ?? 0) >= minBand) return pick(hovered.id, [hovered.id])

  const center = (layer: StackedLayer) => (layer.lower[index] + layer.upper[index]) / 2
  const near = present
    .filter((layer) => Math.abs(center(layer) - value) <= minBand * 1.5)
    .sort((a, b) => Math.abs(center(a) - value) - Math.abs(center(b) - value))
    .slice(0, MAX_TOOLTIP_ROWS)
    .map((layer) => layer.id)
  return pick(hovered.id, near.includes(hovered.id) ? near : [hovered.id, ...near.slice(0, 2)])
}
