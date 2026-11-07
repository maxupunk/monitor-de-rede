import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useDashboardStore, defaultWidgetsOf, widgetSizeOf } from '../../src/stores/dashboard.ts'
import { useAreasStore } from '@/stores/areas'
import { CATALOG, areasSnapshot } from '../helpers/dashboardCatalog'

const DEFAULT_WIDGETS = defaultWidgetsOf(CATALOG)

const postMock = vi.fn()
const getMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: {
    post: (...args: unknown[]) => postMock(...args),
    get: (...args: unknown[]) => getMock(...args),
  },
}))

const LAYOUT_KEY = 'netmonitor_dashboard_layout_v2'

describe('dashboard store', () => {
  let storage: Record<string, string> = {}

  /** A store com o catálogo carregado, como a página a deixa. */
  async function loadedStore() {
    const store = useDashboardStore()
    await store.loadCatalog()
    return store
  }

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
    getMock.mockImplementation((url: string) =>
      url === '/dashboard/catalog' ? Promise.resolve(CATALOG) : Promise.resolve(null)
    )
  })

  it('o padrão vem do catálogo do backend, sem lista própria', async () => {
    const store = await loadedStore()

    expect(getMock).toHaveBeenCalledWith('/dashboard/catalog')
    expect(store.widgets).toHaveLength(12)
    expect(store.visibleWidgets.map((w) => w.id)).toEqual(
      CATALOG.defaultLayout.filter((w) => w.visible).map((w) => w.type)
    )
    expect(store.widgets.find((w) => w.id === 'stat_cards')).toMatchObject({
      title: 'Card stat_cards',
      lg: 12,
    })
    // Só os cards múltiplos viram modelo do "Adicionar card".
    expect(store.cardTemplates.map((t) => t.type)).toEqual([
      'dns_latency',
      'saas_heatmap',
      'cpu_usage',
      'ether_bandwidth',
    ])
  })

  it('pede o catálogo uma vez só', async () => {
    const store = useDashboardStore()
    await Promise.all([store.loadCatalog(), store.loadCatalog()])
    await store.loadCatalog()
    expect(getMock).toHaveBeenCalledTimes(1)
  })

  it('carrega layout salvo no localStorage e complementa com widgets padrão', async () => {
    storage[LAYOUT_KEY] = JSON.stringify([
      { id: 'stat_cards', visible: false, order: 0 },
      { id: 'health_gauge', visible: true, order: 1 },
    ])

    const store = await loadedStore()

    expect(store.visibleWidgets[0].id).toBe('health_gauge')
    expect(store.visibleWidgets.some((w) => w.id === 'stat_cards')).toBe(false)
    expect(store.widgets).toHaveLength(DEFAULT_WIDGETS.length)
  })

  it('card de área desligada fica no layout salvo e volta igual ao religar', async () => {
    const saved = [
      { id: 'docker_summary', type: 'docker_summary', visible: true, order: 0 },
      { id: 'stat_cards', visible: true, order: 1 },
      { id: 'health_gauge', visible: true, order: 2 },
    ]
    storage[LAYOUT_KEY] = JSON.stringify(saved)
    const areas = useAreasStore()
    areas.apply(areasSnapshot({ servers: false }))

    const store = await loadedStore()

    // Desligada: não aparece, nem no "Adicionar card", mas continua no layout.
    expect(store.visibleWidgets.map((w) => w.id)).not.toContain('docker_summary')
    expect(store.availableWidgets.map((w) => w.id)).not.toContain('docker_summary')
    expect(store.widgets.find((w) => w.id === 'docker_summary')).toMatchObject({
      visible: true,
      order: 0,
    })

    // Mexer no layout com a área desligada não a apaga do que se salva.
    store.moveWidget('health_gauge', 'up')
    const persisted = JSON.parse(storage[LAYOUT_KEY]) as Array<{ id: string; order: number }>
    expect(persisted.find((w) => w.id === 'docker_summary')).toBeTruthy()

    areas.apply(areasSnapshot({ servers: true }))
    expect(store.visibleWidgets.map((w) => w.id).slice(0, 3)).toEqual([
      'docker_summary',
      'health_gauge',
      'stat_cards',
    ])
  })

  it('não reintroduz padrão de área desligada que o layout salvo não tinha', async () => {
    storage[LAYOUT_KEY] = JSON.stringify([{ id: 'stat_cards', visible: true, order: 0 }])
    useAreasStore().apply(areasSnapshot({ servers: false, network: false }))

    const store = await loadedStore()

    const ids = store.widgets.map((w) => w.id)
    expect(ids).not.toContain('docker_summary')
    expect(ids).not.toContain('dns_latency')
    expect(ids).toContain('health_gauge')
    expect(store.cardTemplates.map((t) => t.type)).toEqual(['cpu_usage'])
  })

  it('remove widget ocultando padrão e removendo customizado', async () => {
    const store = await loadedStore()
    store.removeWidget('stat_cards')
    expect(store.widgets.find((w) => w.id === 'stat_cards')?.visible).toBe(false)

    const custom = store.addCustomWidget('cpu_usage', { deviceId: 3 })
    expect(custom).toMatchObject({ title: 'Card cpu_usage', lg: 6 })
    expect(store.isDefaultWidget(custom.id)).toBe(false)
    store.removeWidget(custom.id)
    expect(store.widgets.some((w) => w.id === custom.id)).toBe(false)
  })

  it('move widget para cima e para baixo', async () => {
    const store = await loadedStore()
    const idsBefore = store.visibleWidgets.map((w) => w.id)

    store.moveWidget(idsBefore[1], 'up')
    expect(store.visibleWidgets[0].id).toBe(idsBefore[1])

    store.moveWidget(idsBefore[1], 'down')
    expect(store.visibleWidgets[1].id).toBe(idsBefore[1])
  })

  it('reordena widgets pela lista de ids', async () => {
    const store = await loadedStore()
    const original = store.visibleWidgets.map((w) => w.id)
    const reversed = [...original].reverse()

    store.reorderWidgets(reversed)

    expect(store.visibleWidgets.map((w) => w.id)).toEqual(reversed)
  })

  it('reseta para o layout padrão', async () => {
    const store = await loadedStore()
    store.removeWidget('stat_cards')
    store.resetToDefaultLayout()

    expect(store.widgets).toHaveLength(DEFAULT_WIDGETS.length)
    expect(store.widgets.map((w) => w.visible)).toEqual(DEFAULT_WIDGETS.map((w) => w.visible))
  })

  it('alterna modo de edição', () => {
    const store = useDashboardStore()
    expect(store.isEditMode).toBe(false)

    store.toggleEditMode()
    expect(store.isEditMode).toBe(true)

    store.toggleEditMode(false)
    expect(store.isEditMode).toBe(false)
  })

  it('redimensiona pelo tamanho nomeado e lê o tamanho de volta das colunas', async () => {
    const store = await loadedStore()
    store.resizeWidget('health_gauge', 'third')
    const gauge = store.widgets.find((w) => w.id === 'health_gauge')!
    expect([gauge.cols, gauge.md, gauge.lg]).toEqual([12, 6, 4])
    expect(widgetSizeOf(gauge)).toBe('third')

    store.resizeWidget('health_gauge', 'two_thirds')
    expect(widgetSizeOf(gauge)).toBe('two_thirds')
    expect(widgetSizeOf({ md: 12, lg: 12 })).toBe('full')
    expect(widgetSizeOf({ md: 6, lg: 6 })).toBe('half')
  })
})
