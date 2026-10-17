import { describe, expect, it } from 'vitest'
import {
  arrangeCompact,
  arrangeSubtree,
  closedNodes,
  leafColumns,
  leafGrid,
  NODE_BOX,
  spanningForest,
} from '@/utils/topology/treeLayout'
import { edge, node } from './fixtures'

type Box = { x1: number; y1: number; x2: number; y2: number }

function boxOf(pos: { x: number; y: number }): Box {
  return {
    x1: pos.x - NODE_BOX.width / 2,
    y1: pos.y - NODE_BOX.height / 2,
    x2: pos.x + NODE_BOX.width / 2,
    y2: pos.y + NODE_BOX.height / 2,
  }
}

function overlaps(a: Box, b: Box): boolean {
  return a.x1 < b.x2 - 0.5 && b.x1 < a.x2 - 0.5 && a.y1 < b.y2 - 0.5 && b.y1 < a.y2 - 0.5
}

/** Rede de campus: núcleo → N distribuições → M acessos → K folhas cada. */
function campus(dists: number, access: number, leaves: number) {
  const nodes = [node(1, { type: 'switch', name: 'Núcleo' })]
  const edges = []
  let id = 2
  let edgeId = 1
  for (let d = 0; d < dists; d += 1) {
    const dist = id++
    nodes.push(node(dist, { type: 'switch', name: `Dist ${d}` }))
    edges.push(edge(edgeId++, 1, dist))
    for (let a = 0; a < access; a += 1) {
      const acc = id++
      nodes.push(node(acc, { type: 'switch', name: `Acesso ${d}.${a}` }))
      edges.push(edge(edgeId++, dist, acc))
      for (let l = 0; l < leaves; l += 1) {
        const leaf = id++
        nodes.push(node(leaf, { name: `PC ${d}.${a}.${l}` }))
        edges.push(edge(edgeId++, acc, leaf))
      }
    }
  }
  return { nodes, edges }
}

describe('árvore compacta', () => {
  it('grade de folhas: colunas pares quando há mais de uma fileira', () => {
    expect(leafColumns(2)).toBe(2)
    expect(leafColumns(3)).toBe(3)
    expect(leafColumns(5) % 2).toBe(0)
    expect(leafColumns(40)).toBe(6)
    const grid = leafGrid([1, 2, 3, 4, 5])
    expect(grid.positions.size).toBe(5)
    expect(grid.anchorX).toBe(grid.width / 2)
  })

  it('floresta: enlace redundante marca as duas pontas; LAG pai↔filho não', () => {
    const nodes = [node(1, { type: 'router' }), node(2), node(3), node(4)]
    const edges = [edge(1, 1, 2), edge(2, 1, 3), edge(3, 2, 3), edge(4, 1, 4), edge(5, 4, 1)]
    const forest = spanningForest(nodes, edges, [1])
    expect([...forest.crossLinked].sort()).toEqual([2, 3])
    const closed = closedNodes(forest)
    expect(closed.has(4)).toBe(true)
    expect(closed.has(1)).toBe(false)
  })

  it('campus de 800 nós: nenhum nó sobreposto, proporção de tela, pai acima dos filhos', () => {
    const { nodes, edges } = campus(25, 4, 7)
    expect(nodes.length).toBeGreaterThan(800)
    const forest = spanningForest(nodes, edges, [1])
    const block = arrangeCompact((options) => arrangeSubtree(1, forest.children, options))!
    expect(block.positions.size).toBe(nodes.length)

    const boxes = [...block.positions.values()].map(boxOf)
    let collisions = 0
    for (let i = 0; i < boxes.length; i += 1) {
      for (let j = i + 1; j < boxes.length; j += 1)
        if (overlaps(boxes[i], boxes[j])) collisions += 1
    }
    expect(collisions).toBe(0)
    // Nem faixa (o ELK puro dava ~60:1) nem torre.
    const aspect = block.width / block.height
    expect(aspect).toBeGreaterThan(0.6)
    expect(aspect).toBeLessThan(3)
    const inverted = [...forest.parent].filter(
      ([child, parent]) =>
        parent !== null && block.positions.get(parent)!.y >= block.positions.get(child)!.y
    )
    expect(inverted).toEqual([])
  })

  it('com fileiras, o corredor central fica livre para o tronco das arestas', () => {
    const { nodes, edges } = campus(1, 9, 2)
    const forest = spanningForest(nodes, edges, [1])
    const block = arrangeSubtree(2, forest.children, { rowMax: 900 })
    const parent = block.positions.get(2)!
    for (const [id, pos] of block.positions) {
      if (id === 2) continue
      const box = boxOf(pos)
      // Ninguém abaixo do pai ocupa a coluna dele: o tronco desce sem cortar ninguém.
      if (box.y1 > parent.y) expect(box.x1 >= parent.x || box.x2 <= parent.x).toBe(true)
    }
  })
})
