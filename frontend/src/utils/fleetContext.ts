/**
 * O que os equipamentos de uma frota têm agora, lido da última execução da
 * ação de estado — para os formulários das ações partirem disso (redes para
 * escolher, valores atuais dos rádios) em vez de um formulário vazio.
 */
import type { FleetAction } from '@/bindings/FleetAction'
import type { FleetSpec } from '@/bindings/FleetSpec'
import type { MatrixAction } from '@/bindings/MatrixAction'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import { stateColor } from '@/utils/pluginPresentation'

type Record_ = Record<string, unknown>

function isRecord(value: unknown): value is Record_ {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** A saída de cada equipamento que respondeu, por id. */
export function outputsByDevice(batch: PluginBatchView | null | undefined): Map<number, Record_> {
  const outputs = new Map<number, Record_>()
  for (const device of batch?.devices ?? []) {
    if (device.status === 'succeeded' && isRecord(device.output)) {
      outputs.set(device.deviceId, device.output)
    }
  }
  return outputs
}

/** O valor num caminho com pontos (`"_current.radios"`). */
export function valueAt(source: unknown, path: string): unknown {
  return path
    .split('.')
    .reduce<unknown>((value, key) => (isRecord(value) ? value[key] : undefined), source)
}

/** Um valor oferecido: o item como o primeiro equipamento o tem e quem o tem. */
export interface SourceHit {
  item: Record_
  holders: number[]
}

/**
 * Os valores de `"lista.chave"` em todos os equipamentos (ex.: as redes de
 * todos os roteadores), cada um com quem o tem.
 */
export function sourceIndex(outputs: Map<number, Record_>, path: string): Map<string, SourceHit> {
  const index = new Map<string, SourceHit>()
  const dot = path.indexOf('.')
  if (dot < 0) return index
  const list = path.slice(0, dot)
  const key = path.slice(dot + 1)
  for (const [deviceId, output] of outputs) {
    const items = output[list]
    if (!Array.isArray(items)) continue
    for (const item of items) {
      if (!isRecord(item) || item[key] === undefined || item[key] === '') continue
      const value = String(item[key])
      const hit = index.get(value)
      if (hit) hit.holders.push(deviceId)
      else index.set(value, { item, holders: [deviceId] })
    }
  }
  return index
}

/** Os parâmetros de uma ação a partir de um item (mapa parâmetro → campo). */
export function paramsFromItem(target: MatrixAction, item: Record_): Record_ {
  return Object.fromEntries(
    Object.entries(target.params)
      .filter(([, field]) => item[field] !== undefined)
      .map(([param, field]) => [param, item[field]])
  )
}

/** Os valores atuais (`current`) de cada equipamento que os tem. */
export function currentByDevice(outputs: Map<number, Record_>, path: string): Map<number, Record_> {
  const current = new Map<number, Record_>()
  for (const [deviceId, output] of outputs) {
    const values = valueAt(output, path)
    if (isRecord(values)) current.set(deviceId, values)
  }
  return current
}

/** O que abrir no fluxo de uma ação: a ação, como a tela a chama e de onde parte. */
export interface FlowRequest {
  fleetAction: FleetAction
  /** A ação de dispositivo que roda em cada equipamento. */
  action: PluginAction
  /** A leitura que mostra o que a escrita vai mudar (`fleetAction.preview`). */
  previewAction?: PluginAction | null
  title: string
  initialParams?: Record<string, unknown>
  /** Só estes; sem lista, todos os que têm acesso. */
  initialDevices?: number[]
}

/** Um item da lista (ex.: uma rede) com quem o tem. */
export interface ItemRow {
  key: string
  /** O item como o primeiro equipamento que o tem o leu. */
  item: Record<string, unknown>
  /** Os campos do cartão, juntos ("WPA2/WPA3 · 2,4 GHz, 5 GHz"). */
  subtitle: string
  /** Algum equipamento o tem ligado (estado de cor "sucesso"). */
  active: boolean
  entries: { deviceId: number; name: string; state: unknown }[]
  /** Soma do campo de detalhe (ex.: clientes); `null` sem detalhe. */
  detailTotal: number | null
}

/** Os itens da lista da ação de estado, juntando o que cada equipamento tem. */
export function itemRows(
  outputs: Map<number, Record<string, unknown>>,
  matrix: Pick<MatrixLike, 'field' | 'key' | 'state' | 'detail' | 'subtitle'>,
  members: { deviceId: number; name: string }[]
): ItemRow[] {
  const rows = new Map<string, ItemRow>()
  for (const member of members) {
    const list = outputs.get(member.deviceId)?.[matrix.field]
    if (!Array.isArray(list)) continue
    for (const item of list) {
      if (!isRecord(item) || item[matrix.key] === undefined) continue
      const key = String(item[matrix.key])
      let row = rows.get(key)
      if (!row) {
        row = {
          key,
          item,
          subtitle: (matrix.subtitle ?? [])
            .map((field) => item[field])
            .filter((value) => value !== undefined && value !== null && value !== '')
            .join(' · '),
          active: false,
          entries: [],
          detailTotal: matrix.detail ? 0 : null,
        }
        rows.set(key, row)
      }
      const state = item[matrix.state]
      row.entries.push({ deviceId: member.deviceId, name: member.name, state })
      if (stateColor(state) === 'success') row.active = true
      const detail = matrix.detail ? Number(item[matrix.detail]) : NaN
      if (row.detailTotal !== null && Number.isFinite(detail)) row.detailTotal += detail
    }
  }
  return [...rows.values()]
}

interface MatrixLike {
  field: string
  key: string
  state: string
  detail?: string
  subtitle?: string[]
}

/**
 * As ações da aba "Avançado": as de `fleet.tools`, na ordem; sem lista, as que
 * nenhuma outra parte da tela usa (novo, editar, remover, ação do equipamento).
 */
export function toolsOf(fleet: FleetSpec | null | undefined): FleetAction[] {
  const actions = fleet?.actions ?? []
  if (fleet?.tools && fleet.tools.length > 0) {
    return fleet.tools.flatMap((id) => actions.filter((action) => action.id === id))
  }
  const used = new Set(
    [
      fleet?.matrix?.add,
      fleet?.matrix?.edit?.action,
      fleet?.matrix?.remove?.action,
      fleet?.deviceAction,
    ].filter((id): id is string => Boolean(id))
  )
  return actions.filter((action) => !used.has(action.id))
}
