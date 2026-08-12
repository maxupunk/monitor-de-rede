/**
 * Fonte única dos formatadores de exibição (bytes, taxas, datas e tempo
 * relativo). Antes destas funções, cada página/componente carregava a sua
 * própria cópia de `formatBytes`/`formatBps`/`formatDate`, o que fazia a mesma
 * métrica aparecer com casas decimais diferentes conforme a tela.
 */

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'] as const
const BIT_RATE_UNITS = ['bps', 'Kbps', 'Mbps', 'Gbps', 'Tbps'] as const

export interface ScaleFormatOptions {
  /** Casas decimais máximas — zeros à direita são removidos (1.50 ➔ 1.5) */
  fractionDigits?: number
  /** Texto devolvido quando o valor não é um número utilizável */
  fallback?: string
}

/**
 * Reduz o valor à maior unidade em que ele ainda é >= 1 e o arredonda.
 * `base` é 1024 para grandezas de armazenamento e 1000 para taxas de
 * transmissão — a mesma distinção feita pelos coletores SNMP no backend.
 */
function formatScaled(
  value: number | null | undefined,
  base: number,
  units: readonly string[],
  { fractionDigits = 2, fallback }: ScaleFormatOptions = {}
): string {
  const zeroLabel = `0 ${units[0]}`
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return fallback ?? zeroLabel
  }
  if (value === 0) return fallback ?? zeroLabel

  const magnitude = Math.abs(value)
  const index = Math.min(Math.floor(Math.log(magnitude) / Math.log(base)), units.length - 1)
  const scaled = value / base ** index

  return `${Number.parseFloat(scaled.toFixed(fractionDigits))} ${units[index]}`
}

/** Volume de dados em base 1024: 1536 ➔ "1.5 KB" */
export function formatBytes(value?: number | null, options?: ScaleFormatOptions): string {
  return formatScaled(value, 1024, BYTE_UNITS, options)
}

/** Taxa de transmissão em base 1000: 1_500_000 ➔ "1.5 Mbps" */
export function formatBps(value?: number | null, options?: ScaleFormatOptions): string {
  return formatScaled(value, 1000, BIT_RATE_UNITS, options)
}

/**
 * Velocidade negociada de um link. Diferente de uma taxa medida, 0 aqui
 * significa "o equipamento não informou", não "sem tráfego".
 */
export function formatLinkSpeed(bps?: number | null): string {
  if (!bps || bps <= 0) return 'N/A'
  return formatBps(bps, { fractionDigits: 1 })
}

/**
 * Latência em milissegundos: 6.903808999999999 ➔ "6.9 ms", 250.4 ➔ "250 ms".
 *
 * O RTT chega do backend como `f64` cru, com todo o lixo de ponto flutuante da
 * conta que o produziu. Imprimir esse número direto — o que várias telas
 * faziam — enche a interface de 16 dígitos que não significam nada: a medição
 * não tem essa resolução.
 *
 * Acima de 100 ms a casa decimal também é ruído (a diferença entre 250 e 250,4
 * não muda decisão nenhuma), então o valor vira inteiro. Abaixo disso ela
 * importa, porque é onde vive uma rede local.
 */
export function formatLatency(value?: number | null, fallback = 'N/A'): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return fallback
  const digits = Math.abs(value) >= 100 ? 0 : 1
  // `parseFloat` remove o zero à direita (7.0 ➔ 7), como em `formatScaled`.
  return `${Number.parseFloat(value.toFixed(digits))} ms`
}

/**
 * Formata um valor conforme a unidade que veio junto dele na métrica/evento.
 * Unidades desconhecidas são apenas concatenadas.
 */
export function formatMeasuredValue(value: unknown, unit?: string | null): string {
  const numeric = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(numeric)) return String(value ?? '')

  if (unit === 'bytes' || unit === 'B') return formatBytes(numeric)
  if (unit === 'bps') return formatBps(numeric)
  if (unit === 'ms') return formatLatency(numeric)
  return `${numeric}${unit ? ` ${unit}` : ''}`
}

/** Data e hora completas no formato brasileiro: "05/08/2026 14:32:07" */
export function formatDateTime(value?: string | Date | null, fallback = '—'): string {
  const date = toDate(value)
  if (!date) return value ? String(value) : fallback
  return date.toLocaleString('pt-BR')
}

/**
 * Versão compacta usada em tooltips e séries temporais, onde o ano é ruído:
 * "05/08 14:32:07"
 */
export function formatShortDateTime(value?: string | Date | null, fallback = ''): string {
  const date = toDate(value)
  if (!date) return value ? String(value) : fallback
  return date.toLocaleString('pt-BR', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

/** Somente o horário: "14:32:07" */
export function formatClockTime(value?: string | Date | null): string {
  const date = toDate(value) ?? new Date()
  return date.toLocaleTimeString('pt-BR', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

/** Tempo decorrido em português: "há 2 min", "há 3 dias", "nunca" */
export function formatRelativeTime(value?: string | Date | null, emptyLabel = 'nunca'): string {
  const date = toDate(value)
  if (!date) return emptyLabel

  const elapsedSeconds = Math.floor((Date.now() - date.getTime()) / 1000)
  if (elapsedSeconds < 60) return `há ${Math.max(elapsedSeconds, 0)}s`
  if (elapsedSeconds < 3600) return `há ${Math.floor(elapsedSeconds / 60)} min`
  if (elapsedSeconds < 86400) return `há ${Math.floor(elapsedSeconds / 3600)} h`
  return `há ${Math.floor(elapsedSeconds / 86400)} dias`
}

function toDate(value?: string | Date | null): Date | null {
  if (!value) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}
