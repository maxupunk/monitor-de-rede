/**
 * Rótulos de enlace sem sobreposição.
 *
 * O tráfego de um enlace fica no meio dele. No traçado de árvore os irmãos
 * dividem o mesmo barramento, e o meio de vários enlaces cai no mesmo trecho:
 * os rótulos se empilham. Aqui, cada rótulo procura o lugar livre mais
 * próximo do seu — deslizando na vertical, um passo por vez, alternando
 * para baixo e para cima — sem cobrir outro rótulo nem um nó.
 *
 * Tudo em coordenadas do modelo: o rótulo escala junto com o zoom, então o
 * arranjo não muda ao aproximar ou afastar e só é refeito quando muda a
 * posição, o texto ou quais rótulos aparecem.
 */

export interface LabelRect {
  id: string
  x1: number
  y1: number
  x2: number
  y2: number
}

export type Box = Omit<LabelRect, 'id'>

/**
 * O que o rótulo não pode cobrir: os nós e as linhas dos OUTROS enlaces.
 * `owner` é o enlace dono do trecho — o rótulo pode (e deve) ficar sobre a
 * própria linha.
 */
export interface Obstacle extends Box {
  owner?: string
}

/** Quantos passos (para cada lado) um rótulo pode andar até desistir. */
const MAX_STEPS = 8

function overlaps(a: Box, b: Box, gap: number): boolean {
  return a.x1 < b.x2 + gap && b.x1 < a.x2 + gap && a.y1 < b.y2 + gap && b.y1 < a.y2 + gap
}

function shifted(rect: Box, dy: number): Box {
  return { x1: rect.x1, x2: rect.x2, y1: rect.y1 + dy, y2: rect.y2 + dy }
}

/**
 * Caixas finas ao longo de uma linha quebrada — o traçado de um enlace como
 * obstáculo para os rótulos dos outros.
 */
export function polylineBoxes(
  points: { x: number; y: number }[],
  owner: string,
  pad = 2
): Obstacle[] {
  const boxes: Obstacle[] = []
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1]
    const b = points[i]
    boxes.push({
      owner,
      x1: Math.min(a.x, b.x) - pad,
      x2: Math.max(a.x, b.x) + pad,
      y1: Math.min(a.y, b.y) - pad,
      y2: Math.max(a.y, b.y) + pad,
    })
  }
  return boxes
}

/** Lado da célula da grade espacial, em unidades do modelo. */
const CELL = 96

/**
 * Grade espacial: cada caixa entra nas células que cobre, e a consulta só
 * olha as células da caixa procurada. Com centenas de rótulos e milhares de
 * trechos de linha, comparar todos com todos a cada arranjo pesaria; assim
 * cada teste vê só a vizinhança.
 */
class Grid<T extends Box> {
  private readonly cells = new Map<string, T[]>()

  private keys(box: Box): string[] {
    const keys: string[] = []
    for (let cx = Math.floor(box.x1 / CELL); cx <= Math.floor(box.x2 / CELL); cx++)
      for (let cy = Math.floor(box.y1 / CELL); cy <= Math.floor(box.y2 / CELL); cy++)
        keys.push(`${cx}:${cy}`)
    return keys
  }

  insert(item: T) {
    for (const key of this.keys(item)) {
      const cell = this.cells.get(key)
      if (cell) cell.push(item)
      else this.cells.set(key, [item])
    }
  }

  near(box: Box): Set<T> {
    const found = new Set<T>()
    for (const key of this.keys(box)) for (const item of this.cells.get(key) ?? []) found.add(item)
    return found
  }
}

/** 0, +1, −1, +2, −2, … — o lugar original primeiro, depois o mais perto. */
function candidates(step: number): number[] {
  const out = [0]
  for (let i = 1; i <= MAX_STEPS; i++) out.push(i * step, -i * step)
  return out
}

/**
 * Deslocamento vertical de cada rótulo para que nenhum cubra outro rótulo
 * nem um obstáculo (os nós e a linha de outro enlace). A ordem é estável (de cima para baixo, da
 * esquerda para a direita): o mesmo mapa dá sempre o mesmo arranjo, e o
 * primeiro rótulo de um aglomerado fica onde estava. Rótulo sem lugar livre
 * dentro do alcance fica no lugar original — sobreposto é melhor que longe
 * do seu enlace.
 */
export function resolveLabelOverlaps(
  labels: LabelRect[],
  obstacles: Obstacle[] = [],
  gap = 3
): Map<string, number> {
  const order = [...labels].sort((a, b) => a.y1 - b.y1 || a.x1 - b.x1 || a.id.localeCompare(b.id))
  const blocks = new Grid<Obstacle>()
  for (const block of obstacles) blocks.insert(block)
  const placed = new Grid<Box>()
  const result = new Map<string, number>()
  for (const label of order) {
    const step = label.y2 - label.y1 + gap
    const dy =
      candidates(step).find((offset) => {
        const at = shifted(label, offset)
        const around = { x1: at.x1 - gap, x2: at.x2 + gap, y1: at.y1 - gap, y2: at.y2 + gap }
        for (const other of placed.near(around)) if (overlaps(at, other, gap)) return false
        for (const block of blocks.near(at))
          if (block.owner !== label.id && overlaps(at, block, 0)) return false
        return true
      }) ?? 0
    placed.insert(shifted(label, dy))
    result.set(label.id, dy)
  }
  return result
}
