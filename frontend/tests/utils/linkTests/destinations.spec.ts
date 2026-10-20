import { describe, expect, it } from 'vitest'
import {
  destinationSubtitle,
  destinationsFrom,
  measureKind,
  modeOfKind,
  type DestinationNode,
} from '@/utils/linkTests/destinations'
import {
  COLUMN_SPACING,
  LINE_SPACING,
  pathMapPositions,
  verticalMapHeight,
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

describe('mini mapa do teste', () => {
  it('o caminho vai em linha, na ordem medida, com espaço para o resultado', () => {
    expect(pathMapPositions([5, 6, 7])).toEqual({
      5: { x: 0, y: 0 },
      6: { x: LINE_SPACING, y: 0 },
      7: { x: 2 * LINE_SPACING, y: 0 },
    })
    expect(pathMapPositions([])).toEqual({})
  })

  it('no celular o caminho vai em pé, com a altura acompanhando', () => {
    expect(pathMapPositions([5, 6], true)).toEqual({
      5: { x: 0, y: 0 },
      6: { x: 0, y: COLUMN_SPACING },
    })
    expect(verticalMapHeight(2)).toBeLessThan(verticalMapHeight(4))
    expect(verticalMapHeight(40)).toBe(460)
  })
})
