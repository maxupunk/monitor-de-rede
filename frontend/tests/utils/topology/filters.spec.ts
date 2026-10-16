import { describe, expect, it } from 'vitest'
import { activeFilterCount, applyFilters, EMPTY_FILTERS } from '@/utils/topology/filters'
import { nudgeDelta, resolveShortcut } from '@/utils/topology/shortcuts'
import { DEFAULT_PREFS, parsePrefs } from '@/utils/topology/prefs'
import { edge, node } from './fixtures'

const nodes = [
  node(1, { type: 'router', status: 'online' }),
  node(2, { type: 'switch', status: 'online' }),
  node(3, { type: 'workstation', status: 'offline' }),
  node(4, { type: 'camera', status: 'online' }),
]
const edges = [
  edge(10, 1, 2),
  edge(11, 2, 3),
  edge(12, 2, 4, { status: 'down' }),
  edge(13, 1, 4, { linkType: 'wireless' }),
]

describe('filtros do mapa', () => {
  it('sem filtro tudo casa', () => {
    const result = applyFilters(nodes, edges, EMPTY_FILTERS)
    expect(result.active).toBe(false)
    expect(result.nodes.size).toBe(4)
    expect(activeFilterCount(EMPTY_FILTERS)).toBe(0)
  })

  it('por categoria (gateway conta como roteador) e status', () => {
    const result = applyFilters(nodes, edges, {
      ...EMPTY_FILTERS,
      categories: ['router', 'switch'],
    })
    expect([...result.nodes]).toEqual([1, 2])
    expect([...result.edges]).toEqual([10])
    const offline = applyFilters(nodes, edges, { ...EMPTY_FILTERS, statuses: ['offline'] })
    expect([...offline.nodes]).toEqual([3])
  })

  it('"só problemas" traz nó offline, enlace fora e as pontas dele', () => {
    const result = applyFilters(nodes, edges, { ...EMPTY_FILTERS, problemsOnly: true })
    expect([...result.nodes].sort()).toEqual([2, 3, 4])
    expect(result.edges.has(12)).toBe(true)
  })

  it('por tipo de enlace', () => {
    const result = applyFilters(nodes, edges, { ...EMPTY_FILTERS, linkKinds: ['wireless'] })
    expect([...result.edges]).toEqual([13])
  })
})

describe('atalhos de teclado', () => {
  it('traduz teclas em comandos e respeita campos de texto', () => {
    expect(resolveShortcut({ key: 'k', ctrlKey: true })).toBe('search')
    expect(resolveShortcut({ key: 'k', metaKey: true, inEditable: true })).toBe('search')
    expect(resolveShortcut({ key: 'f' })).toBe('fit')
    expect(resolveShortcut({ key: 'f', inEditable: true })).toBeNull()
    expect(resolveShortcut({ key: 'Escape', inEditable: true })).toBe('escape')
    expect(resolveShortcut({ key: '+' })).toBe('zoom-in')
    expect(resolveShortcut({ key: 'Delete' })).toBe('delete')
    expect(resolveShortcut({ key: 'e' })).toBe('toggle-edit')
    expect(resolveShortcut({ key: 'ArrowUp' })).toBe('nudge-up')
    expect(nudgeDelta('nudge-left', true)).toEqual([-40, 0])
    expect(nudgeDelta('fit', false)).toBeNull()
  })
})

describe('preferências do visitante', () => {
  it('lê com segurança e cai no padrão com lixo', () => {
    expect(parsePrefs(null)).toEqual(DEFAULT_PREFS)
    expect(parsePrefs('{quebrado')).toEqual(DEFAULT_PREFS)
    const prefs = parsePrefs(
      JSON.stringify({
        siteId: 3,
        editMode: true,
        filters: { statuses: ['offline', 'bogus'], mode: 'hide', linkKinds: ['vpn'] },
      })
    )
    expect(prefs.siteId).toBe(3)
    expect(prefs.editMode).toBe(true)
    expect(prefs.filters.statuses).toEqual(['offline'])
    expect(prefs.filters.mode).toBe('hide')
    expect(prefs.filters.linkKinds).toEqual(['vpn'])
  })
})
