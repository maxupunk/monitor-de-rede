/**
 * O que os equipamentos de uma frota têm agora, lido da última execução da
 * ação de estado — para os formulários das ações partirem disso (redes para
 * escolher, valores atuais dos rádios) em vez de um formulário vazio.
 */
import type { FleetAction } from '@/bindings/FleetAction'
import type { FleetSpec } from '@/bindings/FleetSpec'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import { actionsUsedBy } from '@/utils/itemList'

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
    [...actionsUsedBy(fleet?.list), fleet?.deviceAction].filter((id): id is string => Boolean(id))
  )
  return actions.filter((action) => !used.has(action.id))
}
