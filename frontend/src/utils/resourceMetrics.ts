export interface ResourceMetricPoint {
  metricName: string
  metricValue: number
  createdAt: string
}

export const RESOURCE_SERIES = {
  cpu: ['cpu_usage'],
  ram: ['memory_used_bytes'],
  memoryUsagePercent: 'memory_usage',
  loadAverage: 'load_average_1m',
  memoryUsedBytes: 'memory_used_bytes',
  memoryTotalBytes: 'memory_total_bytes',
} as const

const WINDOW_MS = {
  '5m': 5 * 60_000,
  '15m': 15 * 60_000,
  '1h': 60 * 60_000,
  '24h': 24 * 60 * 60_000,
} as const

export type ResourceTimeframe = keyof typeof WINDOW_MS

export function latestMetricValue(
  metrics: ResourceMetricPoint[],
  metricName: string
): number | null {
  const match = [...metrics]
    .filter((metric) => metric.metricName === metricName && Number.isFinite(metric.metricValue))
    .sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt))[0]
  return match?.metricValue ?? null
}

export function resourceMetricWindow(
  metrics: ResourceMetricPoint[],
  metricNames: readonly string[],
  timeframe: ResourceTimeframe,
  now = Date.now(),
  maxPoints = 120
): ResourceMetricPoint[] {
  const cutoff = now - WINDOW_MS[timeframe]
  const filtered = metrics
    .filter(
      (metric) =>
        metricNames.includes(metric.metricName) &&
        Number.isFinite(metric.metricValue) &&
        Date.parse(metric.createdAt) >= cutoff
    )
    .sort((left, right) => Date.parse(left.createdAt) - Date.parse(right.createdAt))

  if (maxPoints <= 1) return filtered.slice(-1)
  if (filtered.length <= maxPoints) return filtered
  const lastIndex = filtered.length - 1
  return Array.from({ length: maxPoints }, (_, index) => {
    const sourceIndex = Math.round((index * lastIndex) / (maxPoints - 1))
    return filtered[sourceIndex]
  })
}
