import { describe, expect, it } from 'vitest'
import { latestMetricValue, resourceMetricWindow } from '../src/utils/resourceMetrics.ts'

const NOW = Date.parse('2026-09-01T12:00:00Z')

describe('resource metrics', () => {
  it('usa somente a série real e a janela solicitada', () => {
    const metrics = [
      { metricName: 'cpu_usage', metricValue: 10, createdAt: '2026-09-01T11:40:00Z' },
      { metricName: 'memory_usage', metricValue: 20, createdAt: '2026-09-01T11:59:00Z' },
      { metricName: 'cpu_usage', metricValue: 30, createdAt: '2026-09-01T11:58:00Z' },
    ]

    expect(resourceMetricWindow(metrics, ['cpu_usage'], '5m', NOW)).toEqual([metrics[2]])
  })

  it('não fabrica amostras quando não existe telemetria', () => {
    expect(resourceMetricWindow([], ['memory_usage'], '24h', NOW)).toEqual([])
  })

  it('seleciona o valor real mais recente de uma série auxiliar', () => {
    const metrics = [
      { metricName: 'memory_total_bytes', metricValue: 100, createdAt: '2026-09-01T11:00:00Z' },
      { metricName: 'memory_total_bytes', metricValue: 200, createdAt: '2026-09-01T12:00:00Z' },
    ]
    expect(latestMetricValue(metrics, 'memory_total_bytes')).toBe(200)
    expect(latestMetricValue(metrics, 'load_average_1m')).toBeNull()
  })

  it('reduz séries longas preservando as extremidades', () => {
    const metrics = Array.from({ length: 200 }, (_, index) => ({
      metricName: 'cpu_usage',
      metricValue: index,
      createdAt: new Date(NOW - (199 - index) * 1_000).toISOString(),
    }))
    const window = resourceMetricWindow(metrics, ['cpu_usage'], '5m', NOW, 20)
    expect(window).toHaveLength(20)
    expect(window[0].metricValue).toBe(0)
    expect(window.at(-1)?.metricValue).toBe(199)
  })
})
