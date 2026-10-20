import { describe, expect, it } from 'vitest'
import {
  destinationSubtitle,
  destinationsFrom,
  measureKind,
  modeOfKind,
  type DestinationNode,
} from '@/utils/linkTests/destinations'
import {
  LINE_SPACING,
  pathMapHeight,
  pathMapPositions,
  lightUp,
  pickerPositions,
} from '@/utils/linkTests/pathMap'
import { edge, node } from '../topology/fixtures'

// AP — switch sem IP — BORDA — RB2011, e o servidor ligado à BORDA.
const nodes = [
  node(1, { name: 'AP', ipAddress: '10.0.0.2' }),
  node(2, { name: 'switch', type: 'unmanaged_switch', ipAddress: null }),
  node(3, { name: 'BORDA', ipAddress: '10.0.0.1' }),
  node(4, { name: 'RB2011', ipAddress: '10.0.0.242' }),
  node(9, { name: 'NetMonitor', ipAddress: '10.0.0.170', isSystem: true }),
]
const edges = [edge(11, 1, 2), edge(12, 2, 3), edge(13, 3, 4), edge(14, 9, 3)]
const nodeOf = (id: number) => nodes.find((n) => n.id === id)
const canBeEndpoint = (n: DestinationNode) => Boolean(n.ipAddress) && n.id !== 2

describe('De / Até do teste de velocidade', () => {
  it('vizinho atrás do switch sem IP é enlace direto; o resto, ponta a ponta por menos saltos', () => {
    const list = destinationsFrom(1, nodeOf, edges, [], canBeEndpoint)
    expect(list.map((d) => [d.name, d.direct])).toEqual([
      ['BORDA', true],
      ['NetMonitor', false],
      ['RB2011', false],
    ])
    const borda = list[0]
    expect(borda).toMatchObject({ linkId: null, deviceIds: [1, 2, 3], edgeIds: [11, 12] })
    expect(borda.viaNames).toEqual(['switch'])
    const rb = list.find((d) => d.name === 'RB2011')!
    expect(rb).toMatchObject({ deviceIds: [1, 2, 3, 4], edgeIds: [11, 12, 13], mapped: true })
    expect(rb.viaNames).toEqual(['switch', 'BORDA'])
    // A origem e o switch sem IP nunca são destino.
    expect(list.some((d) => d.deviceId === 1 || d.deviceId === 2)).toBe(false)
  })

  it('equipamento fora do mapa entra no fim, sem caminho', () => {
    const solto = { id: 7, name: 'Solto', ipAddress: '10.0.9.9' }
    const list = destinationsFrom(1, nodeOf, edges, [solto, nodes[2]], canBeEndpoint)
    const last = list[list.length - 1]
    expect(last).toMatchObject({ deviceId: 7, mapped: false, deviceIds: [], edgeIds: [] })
    // BORDA já estava como direto: não repete.
    expect(list.filter((d) => d.deviceId === 3)).toHaveLength(1)
  })

  it('o tipo do teste sai da escolha', () => {
    const direct = { direct: true, linkId: 11 }
    const beyond = { direct: false, linkId: null }
    expect(measureKind(direct, false, 'per_hop')).toEqual({ kind: 'link', linkId: 11 })
    expect(measureKind(beyond, false, 'end_to_end')).toEqual({ kind: 'end_to_end', linkId: null })
    expect(measureKind(beyond, true, 'end_to_end')).toEqual({ kind: 'server', linkId: null })
    expect(measureKind(beyond, true, 'per_hop')).toEqual({ kind: 'path', linkId: null })
    expect(modeOfKind('path')).toBe('per_hop')
    expect(modeOfKind('server')).toBe('end_to_end')
  })

  it('subtítulo diz o que a opção mede', () => {
    const list = destinationsFrom(1, nodeOf, edges, [], canBeEndpoint)
    const via = (names: string[]) => names.join(' › ')
    expect(destinationSubtitle(list[0], via)).toBe('Enlace direto · passando por switch')
    expect(
      destinationSubtitle(
        list.find((d) => d.name === 'RB2011')!,
        via
      )
    ).toBe('Ponta a ponta · 3 saltos · por switch › BORDA')
  })
})

describe('mapas do teste', () => {
  it('recorte: as posições do mapa, crescidas por igual até cada par ter folga no seu eixo', () => {
    // Um abaixo do outro a 100 px: basta a folga vertical (150) — cresce 1,5×.
    expect(
      pathMapPositions([1, 2, 3], { 1: { x: 0, y: 0 }, 2: { x: 0, y: 100 }, 3: { x: 400, y: 100 } })
    ).toEqual({
      1: { x: 0, y: 0 },
      2: { x: 0, y: 150 },
      3: { x: 600, y: 150 },
    })
    // Lado a lado a 115 px: a pílula pede largura (230) — cresce 2×.
    expect(pathMapPositions([1, 2], { 1: { x: 0, y: 0 }, 2: { x: 115, y: 0 } })).toEqual({
      1: { x: 0, y: 0 },
      2: { x: 230, y: 0 },
    })
    // Já afastados: ficam como estão.
    expect(pathMapPositions([1, 2], { 1: { x: 0, y: 0 }, 2: { x: 600, y: 0 } })).toEqual({
      1: { x: 0, y: 0 },
      2: { x: 600, y: 0 },
    })
  })

  it('altura do recorte acompanha o caminho, entre o piso e o teto', () => {
    expect(pathMapHeight({ 1: { x: 0, y: 0 }, 2: { x: 300, y: 0 } })).toBe(240)
    expect(pathMapHeight({ 1: { x: 0, y: 0 }, 2: { x: 0, y: 300 } })).toBe(450)
    expect(pathMapHeight({ 1: { x: 0, y: 0 }, 2: { x: 0, y: 2000 } })).toBe(560)
  })

  it('recorte sem posição de alguém vai em linha', () => {
    expect(pathMapPositions([5, 6], { 5: { x: 1, y: 1 } })).toEqual({
      5: { x: 0, y: 0 },
      6: { x: LINE_SPACING, y: 0 },
    })
    expect(pathMapPositions([], {})).toEqual({})
  })

  it('mapa de escolha: destino possível fica aceso fora do caminho', () => {
    const el = (id: string, classes: string, group: 'nodes' | 'edges' = 'nodes') => ({
      group,
      data: { id },
      classes,
    })
    const lit = lightUp(
      [
        el('n4', 'device path-dimmed rank-core'),
        el('n5', 'device path-dimmed'),
        el('e4', 'path-dimmed', 'edges'),
      ],
      new Set([4])
    )
    expect(lit.map((item) => item.classes)).toEqual([
      'device rank-core',
      'device path-dimmed',
      'path-dimmed',
    ])
  })

  it('mapa de escolha: quem não tem posição vai numa fileira abaixo', () => {
    const positions = pickerPositions([1, 2, 3], { 1: { x: 50, y: 0 }, 2: { x: 300, y: 400 } })
    expect(positions[1]).toEqual({ x: 50, y: 0 })
    expect(positions[3]).toEqual({ x: 50, y: 600 })
  })
})
