import { describe, expect, it } from 'vitest'
import type { ItemList } from '@/bindings/ItemList'
import {
  actionsUsedBy,
  appliesTo,
  deviceRows,
  fleetRows,
  itemActionsOf,
  itemsIn,
  paramsFromItem,
} from '@/utils/itemList'

const redes: ItemList = {
  title: 'Redes',
  itemName: 'rede',
  field: 'networks',
  key: 'ssid',
  layout: 'cards',
  state: 'state',
  detail: 'clients',
  subtitle: ['security', 'bands'],
  add: 'network',
  edit: { action: 'network', params: { ssid: 'ssid', original_ssid: 'ssid' } },
  remove: { action: 'remove_network', params: { ssid: 'ssid' } },
}

const pacotes: ItemList = {
  source: 'list_packages',
  key: 'name',
  layout: 'table',
  toolbar: ['update_index'],
  rowActions: [
    { action: 'install_package', params: { name: 'name' }, hideWhen: 'installed' },
    { action: 'remove_package', params: { name: 'name' }, showWhen: 'installed' },
  ],
}

describe('lista de itens', () => {
  it('na frota, cada item uma vez com quem o tem, o resumo e a soma do detalhe', () => {
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
    const rows = fleetRows(outputs, redes, [
      { deviceId: 2, name: 'AP Sala' },
      { deviceId: 3, name: 'AP Loja' },
    ])
    expect(rows).toHaveLength(1)
    expect(rows[0]?.subtitle).toBe('WPA2 · 2,4 GHz')
    expect(rows[0]?.entries.map((entry) => entry.name)).toEqual(['AP Sala', 'AP Loja'])
    expect(rows[0]?.detailTotal).toBe(4)
    expect(rows[0]?.active).toBe(true)
  })

  it('no equipamento, a saída da ação (ou a lista em field) vira as linhas', () => {
    const output = [
      { name: 'usteer', installed: false },
      { name: 'wpad-mbedtls', installed: true },
      { version: 'sem chave' },
    ]
    const rows = deviceRows(output, pacotes)
    expect(rows.map((row) => row.key)).toEqual(['usteer', 'wpad-mbedtls'])
    expect(rows[0]?.entries).toEqual([])
    expect(itemsIn({ networks: [{ ssid: 'x' }] }, 'networks')).toEqual([{ ssid: 'x' }])
    expect(itemsIn('texto', null)).toEqual([])
  })

  it('o botão vale conforme o item e leva os parâmetros pelo mapa', () => {
    const [install, remove] = pacotes.rowActions ?? []
    expect(install && appliesTo(install, { installed: false })).toBe(true)
    expect(install && appliesTo(install, { installed: true })).toBe(false)
    expect(remove && appliesTo(remove, { installed: true })).toBe(true)
    expect(
      paramsFromItem(
        { params: { ssid: 'ssid', original_ssid: 'ssid', key: 'key' } },
        { ssid: 'Loja' }
      )
    ).toEqual({ ssid: 'Loja', original_ssid: 'Loja' })
  })

  it('os botões ficam na ordem editar, ações, remover — e a lista diz que ações usa', () => {
    expect(itemActionsOf(redes).map((button) => button.kind)).toEqual(['edit', 'remove'])
    expect(itemActionsOf(pacotes).map((button) => button.target.action)).toEqual([
      'install_package',
      'remove_package',
    ])
    expect(actionsUsedBy(redes)).toEqual(['network', 'network', 'remove_network'])
    expect(actionsUsedBy(pacotes)).toEqual(['update_index', 'install_package', 'remove_package'])
    expect(actionsUsedBy(null)).toEqual([])
  })
})
