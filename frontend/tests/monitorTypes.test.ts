import { describe, expect, it } from 'vitest'
import type { Monitor } from '../src/stores/monitors.ts'
import { buildConfiguration, createMonitorForm, monitorToForm } from '../src/utils/monitorTypes.ts'

describe('política adaptativa de latência', () => {
  it('grava os limites, confirmações e contexto da WAN', () => {
    const form = createMonitorForm()
    form.kind = 'http'
    form.target = 'https://chatgpt.com'
    form.latencyAlertMode = 'adaptive'
    form.latencyDeviationPercent = 65
    form.latencyConsecutiveChecks = 4
    form.latencyMinIncreaseMs = 30
    form.latencySourceDeviceId = 9
    form.latencyDownloadCapacityBps = 600_000_000
    form.latencyUploadCapacityBps = 300_000_000

    expect(buildConfiguration(form).latencyAlertPolicy).toEqual({
      mode: 'adaptive',
      deviationPercent: 65,
      consecutiveChecks: 4,
      minIncreaseMs: 30,
      suppressOnSaturation: true,
      saturationThresholdPercent: 80,
      sourceDeviceId: 9,
      downloadCapacityBps: 600_000_000,
      uploadCapacityBps: 300_000_000,
    })
  })

  it('reidrata uma política existente sem perder a capacidade assimétrica', () => {
    const monitor = {
      id: 1,
      deviceId: 1,
      name: 'OpenAI',
      type: 'https',
      target: 'https://chatgpt.com',
      intervalSeconds: 60,
      timeoutSeconds: 5,
      status: 'up',
      isEnabled: true,
      configuration: {
        url: 'https://chatgpt.com',
        latencyAlertPolicy: {
          mode: 'adaptive',
          deviationPercent: 75,
          consecutiveChecks: 5,
          minIncreaseMs: 40,
          suppressOnSaturation: true,
          saturationThresholdPercent: 90,
          sourceDeviceId: 7,
          downloadCapacityBps: 500_000_000,
          uploadCapacityBps: 100_000_000,
        },
      },
    } satisfies Monitor

    const form = monitorToForm(monitor)
    expect(form.latencyDeviationPercent).toBe(75)
    expect(form.latencyConsecutiveChecks).toBe(5)
    expect(form.latencySourceDeviceId).toBe(7)
    expect(form.latencyDownloadCapacityBps).toBe(500_000_000)
    expect(form.latencyUploadCapacityBps).toBe(100_000_000)
  })
})
