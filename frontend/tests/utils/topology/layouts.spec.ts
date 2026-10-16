import { describe, expect, it } from 'vitest'
import {
  buildElkGraph,
  chooseRoots,
  freeSpotAround,
  gridLayout,
  groupLeaves,
  orientEdges,
  placeNewNodes,
  positionsFromElk,
  type ElkNodeJson,
} from '@/utils/topology/layouts'
import { edge, node } from './fixtures'

describe('organizar só os novos', () => {
  const nodes = [node(1, { type: 'router' }), node(2), node(3), node(4)]
  const edges = [edge(1, 1, 2), edge(2, 1, 3)]

  it('não mexe em quem já tem posição e só devolve os novos', () => {
    const positions = { 1: { x: 0, y: 0 }, 2: { x: 300, y: 300 } }
    const placed = placeNewNodes(nodes, edges, positions)
    expect(Object.keys(placed).map(Number).sort()).toEqual([3, 4])
    expect(placed[1]).toBeUndefined()
  })

  it('coloca o novo perto do vizinho posicionado, sem encostar em ninguém', () => {
    const positions = { 1: { x: 1000, y: 1000 }, 2: { x: 1000, y: 1130 } }
    const placed = placeNewNodes(nodes, edges, positions)
    const near = placed[3]
    const distance = Math.hypot(near.x - 1000, near.y - 1000)
    expect(distance).toBeLessThanOrEqual(300)
    for (const pos of Object.values(positions)) {
      expect(Math.hypot(pos.x - near.x, pos.y - near.y)).toBeGreaterThanOrEqual(95)
    }
  })

  it('sem vizinho posicionado, vai para uma fileira abaixo do mapa', () => {
    const positions = { 1: { x: 0, y: 0 }, 2: { x: 0, y: 500 }, 3: { x: 200, y: 200 } }
    const placed = placeNewNodes(nodes, edges, positions)
    expect(placed[4].y).toBeGreaterThan(500)
  })

  it('nó novo ligado a outro nó novo entra depois dele, perto dele', () => {
    const chain = [node(1), node(2), node(3)]
    const placed = placeNewNodes(chain, [edge(1, 1, 2), edge(2, 2, 3)], { 1: { x: 0, y: 0 } })
    expect(Math.hypot(placed[3].x - placed[2].x, placed[3].y - placed[2].y)).toBeLessThan(400)
  })

  it('vaga livre evita posições ocupadas', () => {
    const taken = [{ x: 0, y: 130 }]
    const spot = freeSpotAround({ x: 0, y: 0 }, taken)
    expect(Math.hypot(spot.x - 0, spot.y - 130)).toBeGreaterThanOrEqual(95)
  })
})

describe('layouts automáticos', () => {
  it('raiz: o servidor ligado à rede; senão roteadores; senão o de maior grau', () => {
    const edges = [edge(1, 1, 2), edge(2, 2, 3), edge(3, 2, 4)]
    expect(chooseRoots([node(1, { isSystem: true }), node(2), node(3), node(4)], edges)).toEqual([
      1,
    ])
    expect(
      chooseRoots([node(9, { isSystem: true }), node(2, { type: 'router' }), node(3)], edges)
    ).toEqual([2])
    expect(chooseRoots([node(1), node(2), node(3), node(4)], edges)).toEqual([2])
  })

  it('orienta os enlaces da raiz para a borda', () => {
    const oriented = orientEdges([node(1), node(2), node(3)], [edge(5, 2, 1), edge(6, 3, 2)], [1])
    expect(oriented).toEqual([
      { id: 5, source: 1, target: 2 },
      { id: 6, source: 2, target: 3 },
    ])
  })

  it('agrupa folhas do mesmo vizinho num bloco (a partir de 3)', () => {
    const nodes = [node(1, { type: 'switch' }), node(2, { type: 'router' })]
    const edges = [edge(1, 2, 1)]
    for (let i = 10; i < 15; i += 1) {
      nodes.push(node(i))
      edges.push(edge(i, 1, i))
    }
    const groups = groupLeaves(nodes, edges)
    expect(groups.get(1)?.leaves).toHaveLength(5)
    expect(groups.has(2)).toBe(false)

    const graph = buildElkGraph('hierarchical', nodes, edges, groups)
    const ids = (graph.children ?? []).map((child) => child.id)
    expect(ids).toContain('g1')
    expect(ids).not.toContain('n10')
    expect(graph.edges?.some((e) => e.targets[0] === 'g1' && e.sources[0] === 'n1')).toBe(true)
    expect(graph.layoutOptions?.['elk.algorithm']).toBe('layered')
  })

  it('abre o bloco de folhas em grade ao ler o resultado do ELK', () => {
    const nodes = [node(1, { type: 'switch' }), node(2, { type: 'router' })]
    const edges = [edge(1, 2, 1)]
    for (let i = 10; i < 14; i += 1) {
      nodes.push(node(i))
      edges.push(edge(i, 1, i))
    }
    const groups = groupLeaves(nodes, edges)
    const result: ElkNodeJson = {
      id: 'root',
      children: [
        { id: 'n1', x: 0, y: 0, width: 124, height: 84 },
        { id: 'n2', x: 0, y: 200, width: 124, height: 84 },
        { id: 'g1', x: 500, y: 300, width: 400, height: 200 },
      ],
    }
    const positions = positionsFromElk(result, groups)
    expect(positions[1]).toEqual({ x: 62, y: 42 })
    for (const leaf of groups.get(1)!.leaves) {
      expect(positions[leaf].x).toBeGreaterThanOrEqual(500)
      expect(positions[leaf].y).toBeGreaterThanOrEqual(300)
    }
  })

  it('agrupar por site cria um nó composto por site', () => {
    const graph = buildElkGraph(
      'sites',
      [node(1, { siteId: 1 }), node(2, { siteId: 2 })],
      [edge(1, 1, 2)]
    )
    expect((graph.children ?? []).map((child) => child.id).sort()).toEqual(['s1', 's2'])
    expect(graph.layoutOptions?.['elk.hierarchyHandling']).toBe('INCLUDE_CHILDREN')
  })

  it('grade põe o servidor e a infraestrutura primeiro', () => {
    const positions = gridLayout(
      [node(3, { type: 'workstation' }), node(2, { type: 'router' }), node(1, { isSystem: true })],
      3
    )
    expect(positions[1].x).toBeLessThan(positions[2].x)
    expect(positions[2].x).toBeLessThan(positions[3].x)
  })
})
