import { test } from '@japa/runner'
import { ConfidenceCalculator } from '#modules/topology/confidence_calculator'
import { LinkResolver } from '#modules/topology/link_resolver'
import { TopologyBuilder } from '#modules/topology/topology_builder'

test.group('Network Topology - Unit Tests', () => {
  test('ConfidenceCalculator deve retornar pontuação correta por fonte', ({ assert }) => {
    const calc = new ConfidenceCalculator()
    assert.equal(calc.calculateConfidence('manual'), 100)
    assert.equal(calc.calculateConfidence('lldp'), 95)
    assert.equal(calc.calculateConfidence('cdp'), 90)
    assert.equal(calc.calculateConfidence('snmp'), 80)
    assert.equal(calc.calculateConfidence('inferred'), 50)
  })

  test('LinkResolver deve desduplicar ligações bidirecionais mantendo a de maior confiança', ({
    assert,
  }) => {
    const resolver = new LinkResolver()
    const resolved = resolver.resolveLinks([
      {
        sourceDeviceId: 1,
        targetDeviceId: 2,
        linkType: 'inferred',
        discoveryMethod: 'subnet',
        confidence: 50,
        confirmed: false,
      },
      {
        sourceDeviceId: 2,
        targetDeviceId: 1,
        linkType: 'lldp',
        discoveryMethod: 'snmp_lldp',
        confidence: 95,
        confirmed: false,
      },
    ])

    assert.equal(resolved.length, 1)
    assert.equal(resolved[0].confidence, 95)
    assert.equal(resolved[0].linkType, 'lldp')
  })

  test('TopologyBuilder deve estruturar o grafo com nós e arestas', ({ assert }) => {
    const builder = new TopologyBuilder()
    const graph = builder.buildGraph(
      [
        { id: 1, name: 'Router-01', type: 'router', status: 'online' },
        { id: 2, name: 'Switch-01', type: 'switch', status: 'online' },
      ],
      [
        {
          source: 1,
          target: 2,
          linkType: 'lldp',
          discoveryMethod: 'snmp_lldp',
          confidence: 95,
          confirmed: true,
          status: 'up',
        },
      ]
    )

    assert.equal(graph.nodes.length, 2)
    assert.equal(graph.edges.length, 1)
    assert.equal(graph.edges[0].source, 1)
    assert.equal(graph.edges[0].target, 2)
  })
})
