import { describe, expect, it } from 'vitest'
import { viaText } from '@/utils/linkTests/presentation'
import {
  isLinkTestEndpoint,
  isLinkTestTransparent,
  linkTargets,
  selectedTargetKey,
  transparentEdgeOrigin,
} from '@/utils/linkTests/targets'
import type { TopologyEdge } from '@/stores/topology'
import { edge, node } from '../topology/fixtures'
// O mesmo arquivo que o backend lê (`services/link_tests/transparent.rs`).
import routes from '../../../../backend/tests/fixtures/link_test_routes.json'

/**
 * O cenário do dono do produto, ampliado:
 *
 *   AP-terraço(1) — switch-assistencia(2, sem IP) — BORDA(3)
 *                                      └ switch-b(4, sem IP) — Câmera(5)
 *   AP-terraço(1) — Rádio(6)        (direto)
 *   AP-terraço(1) — Rádio(6)        (segundo cabo, paralelo)
 */
const nodes = new Map([
  [1, node(1, { name: 'AP-terraço', ipAddress: '10.0.0.3' })],
  [2, node(2, { name: 'switch-assistencia', type: 'unmanaged_switch', ipAddress: null })],
  [3, node(3, { name: 'BORDA', ipAddress: '10.0.0.1' })],
  [4, node(4, { name: 'switch-b', type: 'unmanaged_switch', ipAddress: null })],
  [5, node(5, { name: 'Câmera', ipAddress: '10.0.0.50' })],
  [6, node(6, { name: 'Rádio', ipAddress: '10.0.0.6' })],
])
const nodeOf = (id: number) => nodes.get(id)
const edges = [
  edge(11, 1, 2),
  edge(12, 2, 3),
  edge(13, 4, 2),
  edge(14, 4, 5),
  edge(15, 1, 6),
  edge(16, 6, 1),
]

describe('equipamentos sem IP no teste de enlace', () => {
  it('sem IP é transparente; o NetMonitor nunca é', () => {
    expect(isLinkTestTransparent({ ipAddress: null })).toBe(true)
    expect(isLinkTestTransparent({ ipAddress: '  ' })).toBe(true)
    expect(isLinkTestTransparent({})).toBe(true)
    expect(isLinkTestTransparent({ ipAddress: '10.0.0.1' })).toBe(false)
    expect(isLinkTestTransparent({ ipAddress: null, isSystem: true })).toBe(false)
    // Ponta de teste: com IP e não é switch não gerenciável.
    expect(isLinkTestEndpoint({ type: 'router', ipAddress: '10.0.0.1' })).toBe(true)
    expect(isLinkTestEndpoint({ type: 'unmanaged_switch', ipAddress: '10.0.0.9' })).toBe(false)
    expect(isLinkTestEndpoint({ type: 'router', ipAddress: null })).toBe(false)
  })

  it('vizinho sem IP vira os equipamentos com IP depois dele', () => {
    const targets = linkTargets(1, nodeOf, edges)
    expect(targets.map((target) => target.key)).toEqual([
      'device:3', // BORDA
      'device:5', // Câmera
      'edge:15', // Rádio
      'edge:16', // Rádio (segundo cabo)
    ])
    expect(targets[0]).toEqual({
      key: 'device:3',
      deviceId: 3,
      name: 'BORDA',
      linkId: null,
      edgeIds: [11, 12],
      viaDeviceIds: [2],
      viaNames: ['switch-assistencia'],
    })
    // Dois switches seguidos, unidos na legenda.
    expect(targets[1].viaNames).toEqual(['switch-assistencia', 'switch-b'])
    expect(targets[1].edgeIds).toEqual([11, 13, 14])
    expect(viaText(targets[1].viaNames)).toBe('switch-assistencia › switch-b')
    expect(viaText([])).toBeNull()
    // Direto: o enlace que a API recebe.
    expect(targets[2]).toMatchObject({ deviceId: 6, linkId: 15, edgeIds: [15], viaNames: [] })
  })

  it('o próprio equipamento e o switch nunca são destino; direto ganha do trecho', () => {
    // BORDA ligada também direto ao AP: fica só a opção direta.
    const withDirect = [...edges, edge(17, 3, 1)]
    const targets = linkTargets(1, nodeOf, withDirect)
    const borda = targets.filter((target) => target.deviceId === 3)
    expect(borda).toEqual([expect.objectContaining({ key: 'edge:17', linkId: 17 })])
    expect(targets.some((target) => target.deviceId === 1 || target.deviceId === 2)).toBe(false)
    // A partir de BORDA, o AP aparece depois do switch.
    expect(linkTargets(3, nodeOf, edges).map((target) => target.name)).toEqual([
      'AP-terraço',
      'Câmera',
    ])
  })

  it('a opção escolhida a partir do rascunho', () => {
    const targets = linkTargets(1, nodeOf, edges)
    expect(selectedTargetKey({ linkId: 15, targetDeviceId: 6 }, targets)).toBe('edge:15')
    expect(selectedTargetKey({ linkId: null, targetDeviceId: 3 }, targets)).toBe('device:3')
    expect(selectedTargetKey({ linkId: null, targetDeviceId: null }, targets)).toBeNull()
  })

  it('enlace do mapa com uma ponta sem IP parte da outra ponta', () => {
    expect(transparentEdgeOrigin(edge(11, 1, 2), nodeOf)).toEqual({ origin: 1, direct: false })
    expect(transparentEdgeOrigin(edge(12, 2, 3), nodeOf)).toEqual({ origin: 3, direct: false })
    expect(transparentEdgeOrigin(edge(13, 4, 2), nodeOf)).toEqual({ origin: null, direct: false })
    expect(transparentEdgeOrigin(edge(15, 1, 6), nodeOf)).toEqual({ origin: null, direct: true })
    // Nó fora do mapa: segue como enlace direto (o backend decide).
    expect(transparentEdgeOrigin(edge(99, 1, 98), nodeOf)).toEqual({ origin: null, direct: true })
  })

  it('hierarquia declarada é enlace direto e é atravessada, como no backend', () => {
    // Torre(10) ← switch-torre(11, sem IP) — CPE(12); Torre ← Antena(13).
    const map = new Map([
      [10, node(10, { name: 'Torre', ipAddress: '10.0.4.1' })],
      [11, node(11, { name: 'switch-torre', type: 'unmanaged_switch', ipAddress: null })],
      [12, node(12, { name: 'CPE', ipAddress: '10.0.4.2' })],
      [13, node(13, { name: 'Antena', ipAddress: '10.0.4.3' })],
    ])
    const parent = { linkType: 'parent', discoveryMethod: 'parent_hierarchy', confirmed: true }
    const graph = [edge(-11, 10, 11, parent), edge(50, 11, 12), edge(-13, 10, 13, parent)]
    expect(linkTargets(10, (id) => map.get(id), graph)).toEqual([
      expect.objectContaining({ key: 'edge:-13', deviceId: 13, linkId: -13, edgeIds: [-13] }),
      expect.objectContaining({
        key: 'device:12',
        linkId: null,
        edgeIds: [-11, 50],
        viaDeviceIds: [11],
        viaNames: ['switch-torre'],
      }),
    ])
  })

  describe('rotas compartilhadas com o backend (mesmo desempate)', () => {
    const devices = new Map(
      routes.devices.map((device) => [device.id, node(device.id, device)] as const)
    )
    const graph: TopologyEdge[] = routes.edges.map(({ id, source, target, ...rest }) =>
      edge(id, source, target, rest)
    )
    it.each(routes.cases)('$name', ({ from, to, via, edgeIds }) => {
      const target = linkTargets(from, (id) => devices.get(id), graph).find(
        (option) => option.deviceId === to
      )
      expect(target).toMatchObject({ viaDeviceIds: via, edgeIds })
      // A ordem em que as arestas chegam não muda a escolha.
      const reversed = linkTargets(from, (id) => devices.get(id), [...graph].reverse()).find(
        (option) => option.deviceId === to
      )
      expect(reversed).toMatchObject({ viaDeviceIds: via, edgeIds })
    })
  })
})
