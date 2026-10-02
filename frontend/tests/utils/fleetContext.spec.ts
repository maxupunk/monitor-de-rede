import { describe, expect, it } from 'vitest'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import {
  currentByDevice,
  outputsByDevice,
  paramsFromItem,
  sourceIndex,
  valueAt,
} from '@/utils/fleetContext'

function status(): PluginBatchView {
  const device = (deviceId: number, networks: unknown[], channel: string) => ({
    deviceId,
    deviceName: `AP ${deviceId}`,
    runId: deviceId * 10,
    status: 'succeeded',
    error: null,
    output: { networks, _current: { radios: { radio_2g_channel: channel } } },
  })
  return {
    id: 1,
    pluginId: 5,
    action: 'status',
    title: 'Estado',
    status: 'partial',
    devices: [
      device(
        2,
        [
          { ssid: 'Loja', encryption: 'psk2' },
          { ssid: 'OpenWrt', encryption: 'none' },
        ],
        '1'
      ),
      device(3, [{ ssid: 'Loja', encryption: 'psk2' }], '11'),
      { deviceId: 4, deviceName: 'AP 4', runId: 40, status: 'failed', error: 'x', output: null },
    ],
    result: null,
    hasPatch: false,
    error: null,
    createdAt: '2026-10-02T09:00:00Z',
    finishedAt: '2026-10-02T09:00:05Z',
  }
}

describe('contexto da frota', () => {
  it('as redes de todos os roteadores, cada uma com quem a tem', () => {
    const index = sourceIndex(outputsByDevice(status()), 'networks.ssid')
    expect([...index.keys()]).toEqual(['Loja', 'OpenWrt'])
    expect(index.get('Loja')?.holders).toEqual([2, 3])
    expect(index.get('OpenWrt')?.holders).toEqual([2])
    expect(sourceIndex(outputsByDevice(status()), 'semponto').size).toBe(0)
  })

  it('o item escolhido preenche os parâmetros pelo mapa', () => {
    const item = { ssid: 'Loja', encryption: 'psk2' }
    expect(
      paramsFromItem(
        { action: 'network', params: { ssid: 'ssid', original_ssid: 'ssid', key: 'key' } },
        item
      )
    ).toEqual({ ssid: 'Loja', original_ssid: 'Loja' })
  })

  it('valores atuais por equipamento, só de quem respondeu', () => {
    const current = currentByDevice(outputsByDevice(status()), '_current.radios')
    expect(current.get(2)).toEqual({ radio_2g_channel: '1' })
    expect(current.get(3)).toEqual({ radio_2g_channel: '11' })
    expect(current.has(4)).toBe(false)
    expect(valueAt({ a: { b: 1 } }, 'a.c')).toBeUndefined()
  })
})
