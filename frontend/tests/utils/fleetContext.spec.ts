import { describe, expect, it } from 'vitest'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import {
  currentByDevice,
  itemRows,
  toolsOf,
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

describe('telas do aplicativo', () => {
  it('cada item com quem o tem, o resumo do cartão e a soma do detalhe', () => {
    const outputs = new Map<number, Record<string, unknown>>([
      [
        2,
        {
          networks: [
            { ssid: 'Loja', security: 'WPA2', bands: '2,4 GHz', state: 'ativa', clients: 3 },
          ],
        },
      ],
      [
        3,
        {
          networks: [
            { ssid: 'Loja', security: 'WPA2', bands: '2,4 GHz', state: 'desativada', clients: 1 },
          ],
        },
      ],
    ])
    const rows = itemRows(
      outputs,
      {
        field: 'networks',
        key: 'ssid',
        state: 'state',
        detail: 'clients',
        subtitle: ['security', 'bands'],
      },
      [
        { deviceId: 2, name: 'AP Sala' },
        { deviceId: 3, name: 'AP Loja' },
      ]
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.subtitle).toBe('WPA2 · 2,4 GHz')
    expect(rows[0]?.entries.map((entry) => entry.name)).toEqual(['AP Sala', 'AP Loja'])
    expect(rows[0]?.detailTotal).toBe(4)
    expect(rows[0]?.active).toBe(true)
  })

  it('a aba Avançado: a lista declarada ou o que nenhuma parte da tela usa', () => {
    const action = (id: string) => ({ id, title: id, action: id })
    const actions = [action('network'), action('radios'), action('mesh')]
    expect(
      toolsOf({ title: 'x', actions, tools: ['mesh'], matrix: undefined }).map((a) => a.id)
    ).toEqual(['mesh'])
    expect(
      toolsOf({
        title: 'x',
        actions,
        deviceAction: 'radios',
        matrix: { field: 'n', key: 'k', state: 's', add: 'network', subtitle: [] },
      }).map((a) => a.id)
    ).toEqual(['mesh'])
  })
})
