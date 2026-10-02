/**
 * O que os equipamentos de uma frota têm agora, lido da última execução da
 * ação de estado — para os formulários das ações partirem disso (redes para
 * escolher, valores atuais dos rádios) em vez de um formulário vazio.
 */
import type { MatrixAction } from '@/bindings/MatrixAction'
import type { PluginBatchView } from '@/bindings/PluginBatchView'

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
