import type { ModelObject } from '@adonisjs/lucid/types/model'
import Monitor from '#models/monitor'
import Metric from '#models/metric'

/**
 * Monta o payload de listagem de monitores consumido pela UI.
 *
 * Existe como módulo próprio porque **duas** telas exibem a mesma lista —
 * `/monitors` e a aba "Monitores" de `/devices/:id` — e ambas usam o mesmo
 * componente de front. Enquanto o enriquecimento vivia dentro do
 * `MonitorsController`, a tela do equipamento recebia um payload mais pobre
 * (só o último resultado, sem série de uso) e precisava de uma tabela própria,
 * degradada, para não quebrar.
 */

/** Leituras SNMP que são percentual de uso, não checagem up/down */
const GAUGE_METRIC_NAMES = ['cpu_usage', 'memory_usage']

/** Resultados por monitor levados para a linha do tempo da listagem */
export const RECENT_RESULTS_LIMIT = 30

/** Amostras de uso levadas para o sparkline da listagem */
export const GAUGE_HISTORY_LIMIT = 20

export interface GaugeReading {
  name: string
  value: number
  unit: string
  recordedAt: string
}

/**
 * Nome da métrica de uso do monitor, ou `null` quando ele é uma checagem
 * comum de disponibilidade.
 */
export function gaugeMetricName(monitor: Monitor): string | null {
  if (monitor.type !== 'snmp') return null
  const metric = (monitor.configuration as Record<string, unknown> | null)?.metric
  return typeof metric === 'string' && GAUGE_METRIC_NAMES.includes(metric) ? metric : null
}

/**
 * Query base da listagem.
 *
 * `.limit()` sozinho, com múltiplos monitores na mesma consulta, limita o total
 * combinado entre TODOS os monitores (não por monitor) — poucos monitores
 * "consomem" o limite inteiro e os demais ficam quase sem histórico.
 * `.groupLimit()` usa `ROW_NUMBER() OVER (PARTITION BY monitor_id)` para trazer
 * até N resultados de cada um. `.groupOrderBy` vai direto para SQL raw (sem
 * passar pela conversão camelCase ➔ snake_case do Lucid), então precisa do nome
 * da coluna no banco (`started_at`), não da propriedade do model (`startedAt`).
 */
export function monitorListQuery() {
  return Monitor.query().preload('results', (query) =>
    query.groupLimit(RECENT_RESULTS_LIMIT).groupOrderBy('started_at', 'desc')
  )
}

/**
 * Anexa a cada monitor o histórico recente e, quando for monitor de uso, a
 * última leitura de CPU/memória com sua série — para a lista já abrir com a
 * tendência desenhada em vez de só o valor mais recente. A atualização em tempo
 * real via SSE (`applyRealtimeMetrics` no store) então só precisa acrescentar a
 * esse histórico.
 */
export async function presentMonitors(monitors: Monitor[]): Promise<ModelObject[]> {
  const { latestMap, historyMap } = await fetchGaugeMetricsData(monitors)

  return monitors.map((mon) => {
    const json = mon.serialize()
    const results = mon.results || []

    // A UI desenha a linha do tempo do mais antigo para o mais recente
    json.recentResults = [...results].reverse().map((result) => result.serialize())
    json.gaugeMetric = latestMap.get(mon.id) ?? null
    json.gaugeHistory = historyMap.get(mon.id) ?? []
    json.target = mon.target
    json.port = mon.port
    json.latencyMs = results[0]?.latencyMs ?? undefined

    return json
  })
}

export async function fetchGaugeMetricsData(
  monitors: Monitor[],
  historyLimit = GAUGE_HISTORY_LIMIT
): Promise<{
  latestMap: Map<number, GaugeReading>
  historyMap: Map<number, Array<{ value: number; recordedAt: string }>>
}> {
  const gaugeMonitors = monitors
    .map((mon) => ({ mon, metricName: gaugeMetricName(mon) }))
    .filter(
      (entry): entry is { mon: Monitor; metricName: string } =>
        entry.metricName !== null && entry.mon.deviceId !== null
    )

  const latestMap = new Map<number, GaugeReading>()
  const historyMap = new Map<number, Array<{ value: number; recordedAt: string }>>()
  if (gaugeMonitors.length === 0) return { latestMap, historyMap }

  // Uma leitura de uso pertence a (equipamento, métrica) — deduplica antes de
  // consultar para não repetir a mesma busca quando dois monitores apontam para
  // o mesmo par (ex.: dois monitores de CPU no mesmo equipamento).
  const pairs = new Map<string, { deviceId: number; metricName: string }>()
  for (const { mon, metricName } of gaugeMonitors) {
    pairs.set(`${mon.deviceId}:${metricName}`, { deviceId: mon.deviceId!, metricName })
  }

  const rowsByPair = new Map<string, Metric[]>()
  await Promise.all(
    [...pairs.entries()].map(async ([key, { deviceId, metricName }]) => {
      const rows = await Metric.query()
        .where('deviceId', deviceId)
        .where('name', metricName)
        .orderBy('recordedAt', 'desc')
        .limit(historyLimit)
      rowsByPair.set(key, rows)
    })
  )

  for (const { mon, metricName } of gaugeMonitors) {
    const rows = rowsByPair.get(`${mon.deviceId}:${metricName}`) ?? []
    if (rows.length === 0) continue

    // `rows` chega do mais recente para o mais antigo (orderBy desc) — inverte
    // para o histórico ficar do mais antigo para o mais recente, como o resto da UI espera.
    historyMap.set(
      mon.id,
      [...rows].reverse().map((row) => ({ value: row.value, recordedAt: row.recordedAt.toISO()! }))
    )

    const latest = rows[0]
    latestMap.set(mon.id, {
      name: latest.name,
      value: latest.value,
      unit: latest.unit,
      recordedAt: latest.recordedAt.toISO()!,
    })
  }

  return { latestMap, historyMap }
}
