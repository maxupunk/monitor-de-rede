import { describe, expect, it } from 'vitest'
import type { Monitor } from '../src/stores/monitors.ts'
import {
  formatGaugeValue,
  gaugeDisplayUnit,
  gaugeUsagePercent,
} from '../src/utils/monitorPresentation.ts'

function memoryMonitor(): Monitor {
  return {
    id: 1,
    name: 'Memória',
    type: 'snmp',
    deviceId: 1,
    configuration: { metric: 'memory_usage' },
    intervalSeconds: 60,
    timeoutSeconds: 5,
    status: 'up',
    isEnabled: true,
    gaugeMetric: {
      name: 'memory_usage',
      value: 3 * 1024 ** 3,
      totalBytes: 8 * 1024 ** 3,
      usagePercent: 37.5,
      unit: 'bytes',
      recordedAt: '2026-09-01T12:00:00Z',
    },
  }
}

describe('apresentação de memória', () => {
  it('prioriza quantidade usada e total', () => {
    expect(formatGaugeValue(memoryMonitor())).toBe('3 GiB / 8 GiB')
    expect(formatGaugeValue(memoryMonitor(), true)).toBe('3 GiB')
    expect(gaugeDisplayUnit(memoryMonitor())).toBe('bytes')
  })

  it('mantém o percentual somente como metadado auxiliar', () => {
    expect(gaugeUsagePercent(memoryMonitor())).toBe(37.5)
  })
})
