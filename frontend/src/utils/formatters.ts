/**
 * Fonte única dos formatadores de exibição (bytes, taxas, datas e tempo
 * relativo). Antes destas funções, cada página/componente carregava a sua
 * própria cópia de `formatBytes`/`formatBps`/`formatDate`, o que fazia a mesma
 * métrica aparecer com casas decimais diferentes conforme a tela.
 */

const BINARY_BYTE_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'] as const
const DECIMAL_BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'] as const
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

/** Memória e armazenamento em base 1024: 1536 ➔ "1.5 KiB". */
export function formatBinaryBytes(value?: number | null, options?: ScaleFormatOptions): string {
  return formatScaled(value, 1024, BINARY_BYTE_UNITS, options)
}

/** Contadores apresentados pelo Docker em base decimal: 1500 ➔ "1.5 KB". */
export function formatDecimalBytes(value?: number | null, options?: ScaleFormatOptions): string {
  return formatScaled(value, 1000, DECIMAL_BYTE_UNITS, options)
}

/** Compatibilidade para grandezas historicamente tratadas como armazenamento. */
export function formatBytes(value?: number | null, options?: ScaleFormatOptions): string {
  return formatBinaryBytes(value, options)
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

export type SpeedUnit = 'auto' | 'kbps' | 'mbps' | 'gbps'

/** Determina automaticamente a unidade apropriada com base no valor em Mbps (>= 1000 Mbps -> Gbps, >= 1 Mbps -> Mbps, < 1 Mbps -> Kbps) */
export function resolveAutoUnit(speedMbps?: number | null): 'kbps' | 'mbps' | 'gbps' {
  if (
    speedMbps === null ||
    speedMbps === undefined ||
    !Number.isFinite(speedMbps) ||
    speedMbps <= 0
  ) {
    return 'mbps'
  }
  const abs = Math.abs(speedMbps)
  if (abs >= 1000) return 'gbps'
  if (abs < 1) return 'kbps'
  return 'mbps'
}

/** Resolve a unidade final a ser utilizada considerando o modo automático */
export function resolveSpeedUnit(
  valueMbps?: number | null,
  unit: SpeedUnit = 'auto'
): 'kbps' | 'mbps' | 'gbps' {
  if (unit !== 'auto') return unit
  return resolveAutoUnit(valueMbps)
}

/** Converte valor de Mbps para a unidade de velocidade especificada */
export function convertSpeedFromMbps(mbps: number, unit: SpeedUnit): number {
  const resolved = resolveSpeedUnit(mbps, unit)
  switch (resolved) {
    case 'kbps':
      return mbps * 1000
    case 'gbps':
      return mbps / 1000
    case 'mbps':
    default:
      return mbps
  }
}

/** Formata valor numérico de velocidade já convertido para a unidade */
export function formatSpeedValue(
  value?: number | null,
  unit: SpeedUnit = 'mbps',
  fallback = '--'
): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return fallback
  const resolved = unit === 'auto' ? 'mbps' : unit
  const fractionDigits = resolved === 'gbps' ? 2 : resolved === 'kbps' ? 0 : 1
  return Number.parseFloat(value.toFixed(fractionDigits)).toString()
}

/** Formata velocidade a partir de Mbps para a unidade selecionada */
export function formatSpeedByUnit(
  valueMbps?: number | null,
  unit: SpeedUnit = 'auto',
  fallback = '--'
): string {
  if (valueMbps === null || valueMbps === undefined || !Number.isFinite(valueMbps)) return fallback
  const resolved = resolveSpeedUnit(valueMbps, unit)
  const converted = convertSpeedFromMbps(valueMbps, resolved)
  return formatSpeedValue(converted, resolved, fallback)
}

/** Retorna o label formatado da unidade (Kbps, Mbps, Gbps) */
export function getSpeedUnitLabel(unit: SpeedUnit, valueMbps?: number | null): string {
  const resolved = resolveSpeedUnit(valueMbps, unit)
  switch (resolved) {
    case 'kbps':
      return 'Kbps'
    case 'gbps':
      return 'Gbps'
    case 'mbps':
    default:
      return 'Mbps'
  }
}

/** Formata velocidade em Mbps com uma casa decimal: 12.34 ➔ "12.3", null ➔ "--" */
export function formatSpeedMbps(value?: number | null, fallback = '--'): string {
  return formatSpeedByUnit(value, 'mbps', fallback)
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
/** Percentual com casas fixas: `42,5%` (CPU, memória, disco). */
export function formatPercent(value?: number | null, digits = 1, fallback = '—'): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return fallback
  return `${value.toLocaleString('pt-BR', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })}%`
}

/** Contagem compacta: `850`, `1,2 mil`, `3,4 mi` (tokens, amostras, eventos). */
export function formatCompactCount(value?: number | null, fallback = '—'): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return fallback
  if (Math.abs(value) < 1000) return String(Math.round(value))
  const [divisor, suffix] = Math.abs(value) < 1_000_000 ? [1000, 'mil'] : [1_000_000, 'mi']
  return `${(value / divisor).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} ${suffix}`
}

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

/** Somente a data no formato brasileiro: "05/08/2026" */
export function formatDate(value?: string | Date | null, fallback = '—'): string {
  const date = toDate(value)
  if (!date) return value ? String(value) : fallback
  return date.toLocaleDateString('pt-BR')
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

/**
 * Intervalo de tempo formatado em dias e horas: "15 dias e 4 horas", "1 dia", "6 horas", etc.
 */
export function formatTimeSpan(
  startDate?: string | Date | null,
  endDate?: string | Date | null,
  fallback = '—'
): string {
  const start = toDate(startDate)
  const end = toDate(endDate)
  if (!start || !end) return fallback

  const diffMs = Math.max(0, end.getTime() - start.getTime())
  const totalMinutes = Math.floor(diffMs / (1000 * 60))
  const totalHours = Math.floor(totalMinutes / 60)
  const days = Math.floor(totalHours / 24)
  const hours = totalHours % 24
  const minutes = totalMinutes % 60

  if (days === 0 && totalHours === 0) {
    if (minutes === 0) return 'menos de 1 minuto'
    return `${minutes} ${minutes === 1 ? 'minuto' : 'minutos'}`
  }

  if (days === 0) {
    if (minutes === 0) return `${totalHours} ${totalHours === 1 ? 'hora' : 'horas'}`
    return `${totalHours} ${totalHours === 1 ? 'hora' : 'horas'} e ${minutes} min`
  }

  const daysLabel = `${days} ${days === 1 ? 'dia' : 'dias'}`
  if (hours === 0) return daysLabel
  const hoursLabel = `${hours} ${hours === 1 ? 'hora' : 'horas'}`
  return `${daysLabel} e ${hoursLabel}`
}

function toDate(value?: string | Date | null): Date | null {
  if (!value) return null
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}
