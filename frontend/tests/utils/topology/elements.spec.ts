import { describe, expect, it } from 'vitest'
import {
  buildElements,
  edgeElementId,
  nodeElementId,
  parseElementId,
  type TopologyElement,
} from '@/utils/topology/elements'
import { applyFilters, EMPTY_FILTERS } from '@/utils/topology/filters'
import { edge, node } from './fixtures'

function byId(elements: TopologyElement[], id: string): TopologyElement {
  const found = elements.find((el) => el.data.id === id)
  if (!found) throw new Error(`elemento ${id} ausente`)
  return found
}

function classesOf(elements: TopologyElement[], id: string): string[] {
  return byId(elements, id).classes.split(' ').filter(Boolean)
}

describe('buildElements', () => {
  const nodes = [
    node(1, { name: 'Core', type: 'router', isSystem: true, ipAddress: '10.0.0.1' }),
    node(2, { name: 'SW', type: 'switch', status: 'offline' }),
    node(3, { name: 'Hub', type: 'unmanaged_switch', status: 'warning' }),
  ]

  it('CPU e memória: terceira linha só com a opção ligada e coleta recente', () => {
    const withHealth = [
      node(1, { name: 'BORDA', ipAddress: '10.0.0.1', cpuPercent: 37.4, memoryPercent: 91 }),
      node(2, { name: 'AP', ipAddress: '10.0.0.2' }),
      node(3, { name: 'Hub', type: 'unmanaged_switch', cpuPercent: 5 }),
    ]
    const on = buildElements({ nodes: withHealth, edges: [], positions: {}, showHealth: true })
    expect(byId(on, 'n1').data.label).toBe('BORDA\n10.0.0.1\n⚠ CPU 37% · RAM 91%')
    expect(classesOf(on, 'n1')).toContain('three-line')
    // No nível médio o IP sai e a saúde fica (`midLabel` + classe `has-health`).
    expect(byId(on, 'n1').data.midLabel).toBe('BORDA\n⚠ CPU 37% · RAM 91%')
    expect(classesOf(on, 'n1')).toContain('has-health')
    expect(classesOf(on, 'n2')).not.toContain('has-health')
    expect(classesOf(on, 'n1')).not.toContain('two-line')
    // Sem coleta: nada a mais.
    expect(byId(on, 'n2').data.label).toBe('AP\n10.0.0.2')
    expect(classesOf(on, 'n2')).toContain('two-line')
    // Sem IP, a saúde é a segunda linha.
    expect(byId(on, 'n3').data.label).toBe('Hub\nCPU 5%')

    // Os números vão no dado: a camada de selos pinta cada um pela faixa.
    expect(byId(on, 'n1').data).toMatchObject({
      health: '⚠ CPU 37% · RAM 91%',
      cpuPercent: 37.4,
      memoryPercent: 91,
    })
    expect(byId(on, 'n2').data.health).toBeNull()

    const off = buildElements({ nodes: withHealth, edges: [], positions: {}, showHealth: false })
    expect(byId(off, 'n1').data.label).toBe('BORDA\n10.0.0.1')
    expect(byId(off, 'n1').data).toMatchObject({ health: null, cpuPercent: null })
    expect(classesOf(off, 'n1')).toContain('two-line')
  })

  it('mapeia nó com status, tipo, sistema e rótulo de duas linhas quando há IP', () => {
    const elements = buildElements({ nodes, edges: [], positions: { 1: { x: 10, y: 20 } } })
    const core = byId(elements, nodeElementId(1))
    expect(core.group).toBe('nodes')
    expect(core.data.label).toBe('Core\n10.0.0.1')
    expect(core.data.iconKey).toBe('system')
    expect(core.position).toEqual({ x: 10, y: 20 })
    expect(classesOf(elements, 'n1')).toEqual(
      expect.arrayContaining(['status-online', 'system', 'two-line'])
    )
    expect(classesOf(elements, 'n2')).toContain('status-offline')
    expect(classesOf(elements, 'n3')).toEqual(
      expect.arrayContaining(['status-warning', 'unmanaged'])
    )
    expect(byId(elements, 'n2').position).toBeUndefined()
  })

  it('classifica enlaces por tipo, estado, velocidade, uso e defasagem', () => {
    const edges = [
      edge(10, 1, 2, {
        linkType: 'fiber',
        sourceInterfaceSpeed: 10_000_000_000,
        targetInterfaceSpeed: 10_000_000_000,
        inBps: 9_500_000_000,
        outBps: 1_000_000,
      }),
      edge(11, 2, 3, { linkType: 'lldp', status: 'down', stale: true }),
      edge(-3, 1, 3, { linkType: 'parent', discoveryMethod: 'parent_hierarchy' }),
    ]
    const elements = buildElements({ nodes, edges, positions: {} })
    expect(classesOf(elements, edgeElementId(10))).toEqual(
      expect.arrayContaining(['kind-fiber', 'status-up', 'tier-fast', 'util-critical'])
    )
    expect(byId(elements, 'e10').data.trafficLabel).toBe('↓ 9.5 Gbps ↑ 1 Mbps')
    expect(byId(elements, 'e10').data).toMatchObject({ inBps: 9_500_000_000, outBps: 1_000_000 })
    expect(classesOf(elements, 'e11')).toEqual(
      expect.arrayContaining(['kind-ethernet', 'status-down', 'stale', 'tier-unknown'])
    )
    expect(classesOf(elements, 'e-3')).toContain('kind-parent')
    expect(byId(elements, 'e10').data.source).toBe('n1')
    expect(byId(elements, 'e10').data.target).toBe('n2')
  })

  it('descarta enlace com ponta fora do recorte (outro site) e laços', () => {
    const elements = buildElements({
      nodes,
      edges: [edge(20, 1, 99), edge(21, 2, 2)],
      positions: {},
    })
    expect(elements.some((el) => el.data.id === 'e20')).toBe(false)
    expect(elements.some((el) => el.data.id === 'e21')).toBe(false)
  })

  it('marca o que não casa com o filtro como esmaecido ou escondido', () => {
    const edges = [edge(30, 1, 2), edge(31, 1, 3)]
    const filters = { ...EMPTY_FILTERS, statuses: ['offline' as const] }
    const result = applyFilters(nodes, edges, filters)
    const dimmed = buildElements({
      nodes,
      edges,
      positions: {},
      filter: { ...result, mode: 'dim' },
    })
    expect(classesOf(dimmed, 'n1')).toContain('filtered-dimmed')
    expect(classesOf(dimmed, 'n2')).not.toContain('filtered-dimmed')
    expect(classesOf(dimmed, 'e30')).toContain('filtered-dimmed')

    const hidden = buildElements({
      nodes,
      edges,
      positions: {},
      filter: { ...result, mode: 'hide' },
    })
    expect(classesOf(hidden, 'n1')).toContain('filtered-hidden')
  })

  it('cria um nó composto por site quando agrupado', () => {
    const elements = buildElements({
      nodes: [node(1, { siteId: 5, siteName: 'Matriz' }), node(2, { siteId: null })],
      edges: [],
      positions: {},
      groupBySite: true,
    })
    expect(byId(elements, 's5').data.label).toBe('Matriz')
    expect(byId(elements, 'snone').data.label).toBe('Sem site')
    expect(byId(elements, 'n1').data.parent).toBe('s5')
    expect(byId(elements, 'n2').data.parent).toBe('snone')
    // Pais antes dos filhos — o Cytoscape exige.
    expect(elements.findIndex((el) => el.data.id === 's5')).toBeLessThan(
      elements.findIndex((el) => el.data.id === 'n1')
    )
  })

  it('usa o nome do site do cadastro quando o nó não traz siteName', () => {
    const elements = buildElements({
      nodes: [node(1, { siteId: 7 })],
      edges: [],
      positions: {},
      groupBySite: true,
      siteNames: { 7: 'Filial' },
    })
    expect(byId(elements, 's7').data.label).toBe('Filial')
  })
})

describe('parseElementId', () => {
  it('traduz ids do canvas de volta', () => {
    expect(parseElementId('n12')).toEqual({ kind: 'node', id: 12 })
    expect(parseElementId('e-4')).toEqual({ kind: 'edge', id: -4 })
    expect(parseElementId('s3')).toEqual({ kind: 'site', id: 3 })
    expect(parseElementId('__ghost-node')).toBeNull()
  })
})
