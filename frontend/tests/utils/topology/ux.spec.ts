import { describe, expect, it } from 'vitest'
import { createSheetStack, pushSheet, removeSheet } from '@/composables/topology/useSheetStack'
import {
  FIT_MAX_ZOOM,
  fitViewport,
  mapInsets,
  TOPOLOGY_LEGEND_WIDTH,
} from '@/utils/topology/canvasTypes'
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
  }

  it('desconta legenda aberta, minimapa e barra de edição', () => {
    expect(mapInsets(base)).toEqual({ top: 44, right: 64, bottom: 60, left: 0 })
    const busy = mapInsets({
      ...base,
      legendOpen: true,
      minimap: true,
      editMode: true,
    })
    expect(busy.left).toBe(TOPOLOGY_LEGEND_WIDTH + 20)
    expect(busy.bottom).toBeGreaterThan(60)
    expect(busy.top).toBeGreaterThan(44)
    expect(busy.right).toBe(64)
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

describe('enquadrar', () => {
  const box = { x1: 0, y1: 0, x2: 200, y2: 100, w: 200, h: 100 }
  const none = { top: 0, right: 0, bottom: 0, left: 0 }

  it('centra na área livre, descontando os painéis', () => {
    const { zoom, pan } = fitViewport(
      box,
      { width: 1200, height: 800 },
      { ...none, left: 300 },
      0.05
    )
    expect(zoom).toBe(FIT_MAX_ZOOM)
    // Centro da área livre: 300 + 40 + (1200 − 300 − 80) / 2 = 750.
    expect(pan.x).toBeCloseTo(750 - 100 * FIT_MAX_ZOOM)
    expect(pan.y).toBeCloseTo(400 - 50 * FIT_MAX_ZOOM)
  })

  it('pouca área livre (inspetor + painel do teste + legenda): ignora a legenda, sem passar do teto', () => {
    const insets = { top: 44, right: 516, bottom: 60, left: 332 }
    const { zoom, pan } = fitViewport(box, { width: 800, height: 800 }, insets, 0.05)
    // Área sem a legenda: 800 − 516 − 80 = 204 px de largura, à esquerda do painel.
    expect(zoom).toBeCloseTo(204 / 200)
    expect(pan.x).toBeCloseTo(40 + 102 - 100 * (204 / 200))
  })

  it('nem sem a legenda cabe: usa o canvas inteiro', () => {
    const insets = { top: 44, right: 700, bottom: 60, left: 332 }
    const { zoom, pan } = fitViewport(box, { width: 800, height: 800 }, insets, 0.05)
    expect(zoom).toBe(FIT_MAX_ZOOM)
    expect(pan.x).toBeCloseTo(400 - 100 * FIT_MAX_ZOOM)
  })

  it('caixa maior que a tela reduz o zoom, até o mínimo', () => {
    const wide = { x1: 0, y1: 0, x2: 10_000, y2: 100, w: 10_000, h: 100 }
    expect(fitViewport(wide, { width: 1080, height: 800 }, none, 0.05).zoom).toBeCloseTo(0.1)
    expect(fitViewport(wide, { width: 1080, height: 800 }, none, 0.2).zoom).toBe(0.2)
  })
})
