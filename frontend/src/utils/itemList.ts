/**
 * A lista de itens de um plugin (`list` no manifesto) — pacotes de um
 * roteador, redes da frota. Funções puras que transformam a saída das ações
 * nas linhas que `PluginItemList.vue` desenha, no equipamento e na frota.
 *
 * O backend já entrega a lista convertida do formato antigo (`panel`,
 * `fleet.matrix`), então aqui só existe um formato.
 */
import type { Effect } from '@/bindings/Effect'
import type { ItemAction } from '@/bindings/ItemAction'
import type { ItemList } from '@/bindings/ItemList'
import { stateColor } from '@/utils/pluginPresentation'

type Item = Record<string, unknown>

function isItem(value: unknown): value is Item {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** O que a lista sabe de cada ação que os botões citam. */
export interface ListActionInfo {
  id: string
  title: string
  effect: Effect
  icon?: string
}

/** Um equipamento que tem o item (frota). */
export interface ItemEntry {
  deviceId: number
  name: string
  state: unknown
}

/** Um item da lista como a tela o mostra. */
export interface ItemRow {
  key: string
  /** O item como veio (o do primeiro equipamento que o tem, na frota). */
  item: Item
  /** Os campos do cartão, juntos ("WPA2/WPA3 · 2,4 GHz, 5 GHz"). */
  subtitle: string
  /** Ligado: o estado do item (ou de algum equipamento que o tem) é de sucesso. */
  active: boolean
  /** Na frota, quem tem o item; no equipamento, vazio. */
  entries: ItemEntry[]
  /** Soma do campo de detalhe (ex.: clientes); `null` sem detalhe. */
  detailTotal: number | null
}

/** Os itens na saída de uma ação: a própria saída ou a lista em `field`. */
export function itemsIn(output: unknown, field?: string | null): Item[] {
  const list = field ? (isItem(output) ? output[field] : undefined) : output
  return Array.isArray(list) ? list.filter(isItem) : []
}

export function subtitleOf(item: Item, list: Pick<ItemList, 'subtitle'>): string {
  return (list.subtitle ?? [])
    .map((field) => item[field])
    .filter((value) => value !== undefined && value !== null && value !== '')
    .map(String)
    .join(' · ')
}

function newRow(key: string, item: Item, list: ItemList): ItemRow {
  return {
    key,
    item,
    subtitle: subtitleOf(item, list),
    active: false,
    entries: [],
    detailTotal: list.detail ? 0 : null,
  }
}

function addDetail(row: ItemRow, item: Item, list: ItemList) {
  const detail = list.detail ? Number(item[list.detail]) : Number.NaN
  if (row.detailTotal !== null && Number.isFinite(detail)) row.detailTotal += detail
}

/** As linhas de um equipamento: um item por linha, na ordem da saída. */
export function deviceRows(output: unknown, list: ItemList): ItemRow[] {
  return itemsIn(output, list.field)
    .filter((item) => item[list.key] !== undefined && item[list.key] !== null)
    .map((item) => {
      const row = newRow(String(item[list.key]), item, list)
      row.active = list.state ? stateColor(item[list.state]) === 'success' : false
      addDetail(row, item, list)
      return row
    })
}

/** As linhas da frota: cada item uma vez, com os equipamentos que o têm. */
export function fleetRows(
  outputs: Map<number, Item>,
  list: ItemList,
  members: { deviceId: number; name: string }[]
): ItemRow[] {
  const rows = new Map<string, ItemRow>()
  for (const member of members) {
    for (const item of itemsIn(outputs.get(member.deviceId), list.field)) {
      if (item[list.key] === undefined || item[list.key] === null) continue
      const key = String(item[list.key])
      let row = rows.get(key)
      if (!row) {
        row = newRow(key, item, list)
        rows.set(key, row)
      }
      const state = list.state ? item[list.state] : undefined
      row.entries.push({ deviceId: member.deviceId, name: member.name, state })
      if (stateColor(state) === 'success') row.active = true
      addDetail(row, item, list)
    }
  }
  return [...rows.values()]
}

/** Os parâmetros de um botão a partir do item (mapa parâmetro → campo). */
export function paramsFromItem(target: Pick<ItemAction, 'params'>, item: Item): Item {
  return Object.fromEntries(
    Object.entries(target.params)
      .filter(([, field]) => item[field] !== undefined)
      .map(([param, field]) => [param, item[field]])
  )
}

/** O botão vale para este item (`showWhen`/`hideWhen`)? */
export function appliesTo(target: ItemAction, item: Item): boolean {
  if (target.showWhen && item[target.showWhen] !== true) return false
  if (target.hideWhen && item[target.hideWhen] === true) return false
  return true
}

/** Os botões sobre um item, na ordem: editar, outras ações, remover. */
export function itemActionsOf(
  list: ItemList
): { kind: 'edit' | 'row' | 'remove'; target: ItemAction }[] {
  return [
    ...(list.edit ? [{ kind: 'edit' as const, target: list.edit }] : []),
    ...(list.rowActions ?? []).map((target) => ({ kind: 'row' as const, target })),
    ...(list.remove ? [{ kind: 'remove' as const, target: list.remove }] : []),
  ]
}

/** As ações que a lista usa (o resto vai para a aba "Avançado"). */
export function actionsUsedBy(list: ItemList | null | undefined): string[] {
  if (!list) return []
  return [
    list.add,
    ...(list.toolbar ?? []),
    ...itemActionsOf(list).map(({ target }) => target.action),
  ].filter((id): id is string => Boolean(id))
}
