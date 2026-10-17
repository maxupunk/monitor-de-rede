import { describe, expect, it } from 'vitest'
import { createSheetStack, pushSheet, removeSheet } from '@/composables/topology/useSheetStack'
import { mapInsets, TOPOLOGY_LEGEND_WIDTH } from '@/utils/topology/canvasTypes'
import { buildElements, nodeRank } from '@/utils/topology/elements'
import { DEFAULT_PREFS, minimapVisible, parsePrefs } from '@/utils/topology/prefs'
import { edgeRouteClass, lodLevel } from '@/utils/topology/style'
import { node } from './fixtures'

describe('nível de detalhe e traçado', () => {
  it('rótulos: nome+IP de perto, só nome no meio, só infraestrutura de longe, só marcos muito longe', () => {
    expect(lodLevel(1.2)).toBe('near')
    expect(lodLevel(0.7)).toBe('mid')
    expect(lodLevel(0.4)).toBe('far')
    expect(lodLevel(0.1)).toBe('xfar')
  })

  it('o nó leva nome e rótulo completo fixos e a classe de peso', () => {
    const [router] = buildElements({
      nodes: [node(1, { name: 'Borda', type: 'router', ipAddress: '10.0.0.1' })],
      edges: [],
      positions: {},
    })
    expect(router.data.name).toBe('Borda')
    expect(router.data.label).toBe('Borda\n10.0.0.1')
    expect(router.classes).toContain('rank-core')
    expect(nodeRank(node(2, { type: 'switch' }))).toBe('infra')
    expect(nodeRank(node(3, { type: 'workstation' }))).toBe('edge')
    expect(nodeRank(node(4, { isSystem: true }))).toBe('core')
  })

  it('enlace vira ortogonal só quando uma ponta está bem acima da outra', () => {
    expect(edgeRouteClass({ x: 0, y: 0 }, { x: 300, y: 150 })).toBe('route-down')
    expect(edgeRouteClass({ x: 0, y: 150 }, { x: 300, y: 0 })).toBe('route-up')
    expect(edgeRouteClass({ x: 0, y: 0 }, { x: 300, y: 20 })).toBeNull()
  })
})

describe('painéis flutuantes e enquadramento', () => {
  const base = {
    narrow: false,
    editMode: false,
    legendOpen: false,
    minimap: false,
    sheetHeight: 0,
    panelRight: 0,
  }

  it('desconta legenda aberta, minimapa, barra de edição e painel do teste', () => {
    expect(mapInsets(base)).toEqual({ top: 44, right: 64, bottom: 60, left: 0 })
    const busy = mapInsets({
      ...base,
      legendOpen: true,
      minimap: true,
      editMode: true,
      panelRight: 516,
    })
    expect(busy.left).toBe(TOPOLOGY_LEGEND_WIDTH + 20)
    expect(busy.bottom).toBeGreaterThan(60)
    expect(busy.top).toBeGreaterThan(44)
    expect(busy.right).toBe(516)
  })

  it('no celular, só a folha de baixo (ou o resumo) conta', () => {
    expect(mapInsets({ ...base, narrow: true, legendOpen: true, sheetHeight: 500 })).toEqual({
      top: 0,
      right: 0,
      bottom: 500,
      left: 0,
    })
  })
})

describe('folhas do celular: uma por vez', () => {
  it('a última aberta vence e fechar devolve a anterior', () => {
    const stack = createSheetStack()
    stack.open('inspector')
    stack.open('link-test')
    expect(stack.top).toBe('link-test')
    stack.close('link-test')
    expect(stack.top).toBe('inspector')
    stack.open('inspector')
    stack.close('inspector')
    expect(stack.top).toBeNull()
  })

  it('a pilha nunca repete uma folha', () => {
    expect(pushSheet(['inspector', 'options'], 'inspector')).toEqual(['options', 'inspector'])
    expect(removeSheet(['inspector', 'options'], 'options')).toEqual(['inspector'])
  })
})

describe('preferências de primeira visita', () => {
  it('a legenda abre só na primeira visita', () => {
    expect(parsePrefs(null).legendOpen).toBe(true)
    expect(parsePrefs(JSON.stringify({ legendSeen: true, legendOpen: true })).legendOpen).toBe(
      false
    )
    expect(DEFAULT_PREFS.orthogonalEdges).toBe(true)
    expect(parsePrefs(JSON.stringify({ orthogonalEdges: false })).orthogonalEdges).toBe(false)
  })

  it('minimapa automático só em mapa grande; a escolha do visitante vence', () => {
    expect(minimapVisible('auto', 50)).toBe(false)
    expect(minimapVisible('auto', 800)).toBe(true)
    expect(minimapVisible('off', 800)).toBe(false)
    expect(minimapVisible('on', 5)).toBe(true)
    expect(parsePrefs(JSON.stringify({ minimap: 'bogus' })).minimap).toBe('auto')
  })
})
