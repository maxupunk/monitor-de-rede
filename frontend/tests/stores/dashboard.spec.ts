import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useDashboardStore, DEFAULT_WIDGETS } from '../../src/stores/dashboard.ts'

const postMock = vi.fn()
const getMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: {
    post: (...args: unknown[]) => postMock(...args),
    get: (...args: unknown[]) => getMock(...args),
  },
}))

describe('dashboard store', () => {
  let storage: Record<string, string> = {}

  beforeEach(() => {
    setActivePinia(createPinia())
    storage = {}
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage[key] ?? null,
      setItem: (key: string, value: string) => {
        storage[key] = value
      },
      removeItem: (key: string) => {
        delete storage[key]
      },
    })
    postMock.mockReset()
    getMock.mockReset()
  })

  it('inicia com widgets padrão quando não há layout salvo', () => {
    const store = useDashboardStore()

    expect(store.widgets).toHaveLength(DEFAULT_WIDGETS.length)
    expect(store.visibleWidgets.map((w) => w.id)).toEqual(DEFAULT_WIDGETS.map((w) => w.id))
  })

  it('carrega layout salvo no localStorage e complementa com widgets padrão', () => {
    storage['netmonitor_dashboard_layout_v1'] = JSON.stringify([
      { id: 'stat_cards', visible: false, order: 0 },
      { id: 'health_gauge', visible: true, order: 1 },
    ])

    const store = useDashboardStore()

    expect(store.visibleWidgets[0].id).toBe('health_gauge')
    expect(store.visibleWidgets.some((w) => w.id === 'stat_cards')).toBe(false)
    expect(store.widgets).toHaveLength(DEFAULT_WIDGETS.length)
  })

  it('remove widget ocultando padrão e removendo customizado', () => {
    const store = useDashboardStore()
    store.removeWidget('stat_cards')

    const stat = store.widgets.find((w) => w.id === 'stat_cards')
    expect(stat?.visible).toBe(false)
  })

  it('move widget para cima e para baixo', () => {
    const store = useDashboardStore()
    const idsBefore = store.visibleWidgets.map((w) => w.id)

    store.moveWidget(idsBefore[1], 'up')
    expect(store.visibleWidgets[0].id).toBe(idsBefore[1])

    store.moveWidget(idsBefore[1], 'down')
    expect(store.visibleWidgets[1].id).toBe(idsBefore[1])
  })

  it('reordena widgets pela lista de ids', () => {
    const store = useDashboardStore()
    const original = store.visibleWidgets.map((w) => w.id)
    const reversed = [...original].reverse()

    store.reorderWidgets(reversed)

    expect(store.visibleWidgets.map((w) => w.id)).toEqual(reversed)
  })

  it('reseta para o layout padrão', () => {
    const store = useDashboardStore()
    store.removeWidget('stat_cards')
    store.resetToDefaultLayout()

    expect(store.widgets).toHaveLength(DEFAULT_WIDGETS.length)
    expect(store.widgets.every((w) => w.visible)).toBe(true)
  })

  it('alterna modo de edição', () => {
    const store = useDashboardStore()
    expect(store.isEditMode).toBe(false)

    store.toggleEditMode()
    expect(store.isEditMode).toBe(true)

    store.toggleEditMode(false)
    expect(store.isEditMode).toBe(false)
  })
})
