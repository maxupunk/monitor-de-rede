/**
 * Posições dos mapas do teste. Puro.
 *
 * - **Recorte do caminho** (`pathMapPositions`, tela do teste): as posições do
 *   próprio mapa — o desenho que o operador conhece. Vizinhos de caminho muito
 *   próximos espremiam a pílula com os Mbps sobre os ícones (visto na rede
 *   real), então o desenho inteiro cresce por igual até o par mais próximo
 *   ter folga: mesma forma, mais espaço. Sem posição de alguém, em linha.
 * - **Mapa de escolha** (`pickerPositions`, assistente): todos os
 *   equipamentos nas posições do mapa; quem ainda não tem posição vai numa
 *   fileira abaixo, para continuar clicável.
 */
import type { TopologyPosition } from '@/stores/topology'
import { parseElementId, type TopologyElement } from '@/utils/topology/elements'

/**
 * Folga mínima entre vizinhos do caminho. A pílula "↓ 999 Mbps ↑ 999 Mbps" é
 * deitada: lado a lado ela pede largura; um abaixo do outro, só a altura do
 * ícone, do nome e dela.
 */
export const MIN_GAP_X = 230
export const MIN_GAP_Y = 150
/** Espaço entre equipamentos quando não há posição no mapa. */
export const LINE_SPACING = 260

type Layout = Readonly<Record<number, TopologyPosition | undefined>>

function inLine(nodeIds: ReadonlyArray<number>, y = 0, x0 = 0): Record<number, TopologyPosition> {
  return Object.fromEntries(nodeIds.map((id, index) => [id, { x: x0 + index * LINE_SPACING, y }]))
}

export function pathMapPositions(
  nodeIds: ReadonlyArray<number>,
  layout: Layout
): Record<number, TopologyPosition> {
  if (nodeIds.length === 0) return {}
  const known = nodeIds.map((id) => layout[id])
  if (known.some((position) => !position)) return inLine(nodeIds)
  const positions = known as TopologyPosition[]
  // Quanto cada par de vizinhos precisa crescer — pelo eixo que custa menos.
  let scale = 1
  for (let i = 1; i < positions.length; i++) {
    const dx = Math.abs(positions[i].x - positions[i - 1].x)
    const dy = Math.abs(positions[i].y - positions[i - 1].y)
    const byX = dx > 0 ? MIN_GAP_X / dx : Infinity
    const byY = dy > 0 ? MIN_GAP_Y / dy : Infinity
    const needed = Math.min(byX, byY)
    if (Number.isFinite(needed)) scale = Math.max(scale, needed)
  }
  return Object.fromEntries(
    nodeIds.map((id, index) => [
      id,
      { x: positions[index].x * scale, y: positions[index].y * scale },
    ])
  )
}

export function pickerPositions(
  nodeIds: ReadonlyArray<number>,
  layout: Layout
): Record<number, TopologyPosition> {
  const placed = nodeIds.filter((id) => layout[id])
  const missing = nodeIds.filter((id) => !layout[id])
  const result: Record<number, TopologyPosition> = {}
  for (const id of placed) result[id] = { ...(layout[id] as TopologyPosition) }
  if (missing.length === 0) return result
  const ys = placed.map((id) => result[id].y)
  const xs = placed.map((id) => result[id].x)
  const below = ys.length > 0 ? Math.max(...ys) + 200 : 0
  const left = xs.length > 0 ? Math.min(...xs) : 0
  return { ...result, ...inLine(missing, below, left) }
}

/** Altura do recorte: acompanha a altura do caminho desenhado, entre o piso e o teto. */
export function pathMapHeight(
  positions: Readonly<Record<number, TopologyPosition>>,
  floor = 240,
  ceiling = 560
): number {
  const ys = Object.values(positions).map((p) => p.y)
  const tall = ys.length > 0 ? Math.max(...ys) - Math.min(...ys) : 0
  return Math.round(Math.min(Math.max(tall + 150, floor), ceiling))
}

/**
 * Mapa de escolha: quem pode ser destino fica aceso mesmo fora do caminho
 * destacado — é o que se toca. O resto continua esmaecido.
 */
export function lightUp(
  elements: ReadonlyArray<TopologyElement>,
  pickable: ReadonlySet<number>
): TopologyElement[] {
  return elements.map((element) => {
    const id = parseElementId(element.data.id)
    if (element.group !== 'nodes' || id?.kind !== 'node' || !pickable.has(id.id)) return element
    return {
      ...element,
      classes: element.classes
        .split(' ')
        .filter((name) => name !== 'path-dimmed')
        .join(' '),
    }
  })
}
