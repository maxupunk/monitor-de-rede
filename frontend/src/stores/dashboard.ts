import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'
import type { Area } from '@/bindings/Area'
import type { DashboardCatalog } from '@/bindings/DashboardCatalog'
import type { WidgetCatalogEntry } from '@/bindings/WidgetCatalogEntry'
import { useAreasStore } from './areas'

export type WidgetCategory = 'summary' | 'lists' | 'charts'
export type SyncMode = 'server' | 'local'
export type ResourceCompatibilityType =
  'bandwidth' | 'dual-axis' | 'numeric' | 'binary' | 'dns-resolvers'

export interface WidgetCustomConfig {
  deviceId?: number | 'all' | null
  interfaceId?: number | 'all' | null
  interfaceName?: string | null
  monitorId?: number | 'all' | null
  targetHost?: string | null
  dnsServerIds?: number[]
  timeframe?: '5m' | '15m' | '1h' | '24h'
  chartType?: 'line' | 'area' | 'bar' | 'gauge'
  unit?: string
  warningThreshold?: number
  criticalThreshold?: number
  [key: string]: unknown
}

export interface WidgetConfig {
  id: string
  type?: string
  title: string
  category: WidgetCategory
  cols?: number
  sm?: number
  md?: number
  lg?: number
  visible: boolean
  order: number
  description: string
  icon: string
  config?: WidgetCustomConfig
}

/**
 * Um tipo de card que se adiciona várias vezes, cada um com a sua
 * configuração. Vem do catálogo único do backend (`GET /dashboard/catalog`,
 * ARQ-04): a tela não mantém lista própria de tipos, títulos nem ícones.
 */
export type CardTemplate = WidgetCatalogEntry

const CATEGORIES: readonly WidgetCategory[] = ['summary', 'lists', 'charts']

function asCategory(value: string | undefined): WidgetCategory {
  return CATEGORIES.find((category) => category === value) ?? 'charts'
}

/*
 * v2: o padrão de fábrica mudou (enxuto, com o Docker como widget). Quem tinha
 * só o cache local passa a ver o padrão novo; o layout salvo no servidor
 * continua valendo para quem sincroniza.
 */
const STORAGE_KEY = 'netmonitor_dashboard_layout_v2'
const SYNC_MODE_KEY = 'netmonitor_dashboard_sync_mode'
const PROMPT_DISMISSED_KEY = 'netmonitor_dashboard_prompt_dismissed'

/** Larguras nomeadas: o que o operador (e a IA) escolhe, em vez de colunas. */
export type WidgetSize = 'third' | 'half' | 'two_thirds' | 'full'

export const WIDGET_SIZES: Record<
  WidgetSize,
  { label: string; icon: string; cols: number; sm: number; md: number; lg: number }
> = {
  third: { label: '1/3 da largura', icon: 'mdi-view-column', cols: 12, sm: 12, md: 6, lg: 4 },
  half: { label: 'Metade', icon: 'mdi-view-split-vertical', cols: 12, sm: 12, md: 6, lg: 6 },
  two_thirds: { label: '2/3 da largura', icon: 'mdi-view-quilt', cols: 12, sm: 12, md: 12, lg: 8 },
  full: { label: 'Largura total', icon: 'mdi-view-stream', cols: 12, sm: 12, md: 12, lg: 12 },
}

/** Tamanho nomeado de um widget, deduzido das colunas salvas. */
export function widgetSizeOf(widget: Pick<WidgetConfig, 'md' | 'lg'>): WidgetSize {
  const lg = widget.lg ?? widget.md ?? 12
  if (lg <= 4) return 'third'
  if (lg <= 6) return 'half'
  if (lg <= 8) return 'two_thirds'
  return 'full'
}

function sized(size: WidgetSize) {
  const { cols, sm, md, lg } = WIDGET_SIZES[size]
  return { cols, sm, md, lg }
}

/** Um card a partir da entrada do catálogo. */
function fromCatalog(entry: WidgetCatalogEntry): Omit<WidgetConfig, 'id' | 'visible' | 'order'> {
  return {
    type: entry.type,
    title: entry.title,
    category: asCategory(entry.category),
    ...sized(entry.defaultSize),
    description: entry.description,
    icon: entry.icon,
  }
}

/**
 * O layout de fábrica, na ordem do catálogo. O padrão responde, de cima para
 * baixo: "está tudo bem?" (números), "o que está quebrado?" (saúde e
 * alertas), "como a rede se comporta?" (latência e eventos), "quem oscila?" e
 * o detalhe por monitor. Os de `visible: false` ficam no layout, prontos para
 * ligar.
 */
export function defaultWidgetsOf(catalog: DashboardCatalog | null): WidgetConfig[] {
  if (!catalog) return []
  return catalog.defaultLayout.flatMap((item, order) => {
    const entry = catalog.widgets.find((widget) => widget.type === item.type)
    if (!entry) return []
    return [
      { ...fromCatalog(entry), ...sized(item.size), id: item.type, visible: item.visible, order },
    ]
  })
}

export const useDashboardStore = defineStore('dashboard', () => {
  const isEditMode = ref(false)
  const syncMode = ref<SyncMode>((localStorage.getItem(SYNC_MODE_KEY) as SyncMode) || 'server')
  const promptDismissed = ref(localStorage.getItem(PROMPT_DISMISSED_KEY) === 'true')
  const showServerPrompt = ref(false)
  const clientIdSession = ref('client-' + Math.random().toString(36).substring(2, 10))

  const serverLayoutData = ref<WidgetConfig[] | null>(null)
  const savingGlobal = ref(false)
  const loadingServer = ref(false)

  const areasStore = useAreasStore()
  const catalog = ref<DashboardCatalog | null>(null)
  /** O catálogo já foi pedido (com ou sem sucesso): a grade pode desenhar. */
  const ready = ref(false)
  let catalogRequest: Promise<void> | null = null

  // O layout só se monta com o catálogo: é dele que vêm os padrões.
  const widgets = ref<WidgetConfig[]>([])

  function catalogEntry(type: string | undefined): WidgetCatalogEntry | undefined {
    return catalog.value?.widgets.find((entry) => entry.type === type)
  }

  /** A área do card; tipo desconhecido conta como núcleo (sempre ligado). */
  function areaOfWidget(widget: Pick<WidgetConfig, 'id' | 'type'>): Area {
    return catalogEntry(widget.type || widget.id)?.area ?? 'core'
  }

  function isWidgetAreaOn(widget: Pick<WidgetConfig, 'id' | 'type'>): boolean {
    return areasStore.isEnabled(areaOfWidget(widget))
  }

  function defaultWidgets(): WidgetConfig[] {
    return defaultWidgetsOf(catalog.value)
  }

  /** Card do layout de fábrica (oculta-se em vez de sair do layout). */
  function isDefaultWidget(id: string): boolean {
    return catalog.value?.defaultLayout.some((item) => item.type === id) ?? false
  }

  /** Modelos de card múltiplo para o diálogo "Adicionar" — só de área ligada. */
  const cardTemplates = computed<CardTemplate[]>(() =>
    (catalog.value?.widgets ?? []).filter(
      (entry) => entry.allowMultiple && areasStore.isEnabled(entry.area)
    )
  )

  /**
   * Pede o catálogo uma vez (ação pontual, não polling) e monta o layout
   * salvo em cima dele.
   */
  function loadCatalog(): Promise<void> {
    catalogRequest ??= (async () => {
      try {
        catalog.value = await apiService.get<DashboardCatalog>('/dashboard/catalog')
      } catch {
        // Sem catálogo, o layout salvo ainda se desenha; só faltam os padrões.
      } finally {
        widgets.value = loadInitialLayout()
        ready.value = true
      }
    })()
    return catalogRequest
  }

  function parseSavedList(savedList: Partial<WidgetConfig>[]): WidgetConfig[] {
    const merged: WidgetConfig[] = []
    const processedIds = new Set<string>()
    const defaults = defaultWidgets()

    savedList.forEach((savedItem, index) => {
      if (!savedItem.id) return

      // Procura primeiro no layout de fábrica
      const def = defaults.find((w) => w.id === savedItem.id)
      if (def) {
        merged.push({
          ...def,
          ...savedItem,
          type: savedItem.type || def.type || def.id,
          visible: savedItem.visible ?? def.visible,
          order: typeof savedItem.order === 'number' ? savedItem.order : index,
        })
        processedIds.add(def.id)
      } else {
        // É um card dinâmico customizado criado pelo usuário
        const entry = catalogEntry(savedItem.type)
        const tmpl = entry ? fromCatalog(entry) : null
        const baseCategory = tmpl ? tmpl.category : savedItem.category || 'charts'
        const baseIcon = tmpl ? tmpl.icon : savedItem.icon || 'mdi-view-dashboard-customize'
        const baseCols = tmpl ?? sized('half')

        merged.push({
          id: savedItem.id,
          type: savedItem.type,
          title: savedItem.title || (tmpl ? tmpl.title : 'Card Personalizado'),
          category: baseCategory,
          cols: savedItem.cols || baseCols.cols,
          sm: savedItem.sm || baseCols.sm,
          md: savedItem.md || baseCols.md,
          lg: savedItem.lg || baseCols.lg,
          visible: savedItem.visible ?? true,
          order: typeof savedItem.order === 'number' ? savedItem.order : index,
          description: savedItem.description || (tmpl ? tmpl.description : ''),
          icon: baseIcon,
          config: savedItem.config || {},
        })
        processedIds.add(savedItem.id)
      }
    })

    // Completa com os padrões que faltam — menos os de área desligada: o
    // layout salvo volta exatamente como estava quando a área religar.
    defaults.forEach((def) => {
      if (!processedIds.has(def.id) && isWidgetAreaOn(def)) {
        merged.push({ ...def, order: merged.length })
      }
    })

    merged.sort((a, b) => a.order - b.order)
    return merged
  }

  function loadInitialLayout(): WidgetConfig[] {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      if (!raw) return defaultWidgets()
      const savedList = JSON.parse(raw) as Partial<WidgetConfig>[]
      if (!Array.isArray(savedList)) return defaultWidgets()
      return parseSavedList(savedList)
    } catch {
      return defaultWidgets()
    }
  }

  function getExportableList(list: WidgetConfig[]) {
    return list.map((w) => ({
      id: w.id,
      type: w.type || w.id,
      title: w.title,
      category: w.category,
      cols: w.cols,
      sm: w.sm,
      md: w.md,
      lg: w.lg,
      visible: w.visible,
      order: w.order,
      description: w.description,
      icon: w.icon,
      config: w.config || {},
    }))
  }

  function saveLocalLayoutCache() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(getExportableList(widgets.value)))
    } catch {
      // Ignora erro no localStorage
    }
  }

  const sortedWidgets = computed(() => {
    return [...widgets.value].sort((a, b) => a.order - b.order)
  })

  /**
   * Cards de área ligada. Os de área desligada continuam no layout (e no que
   * se salva), só não aparecem.
   */
  const availableWidgets = computed(() => sortedWidgets.value.filter(isWidgetAreaOn))

  const visibleWidgets = computed(() => {
    return availableWidgets.value.filter((w) => w.visible)
  })

  const hiddenWidgets = computed(() => {
    return availableWidgets.value.filter((w) => !w.visible)
  })

  /**
   * Busca o layout global salvo no backend
   */
  async function fetchServerLayout(): Promise<WidgetConfig[] | null> {
    loadingServer.value = true
    try {
      const res = await apiService.get<{
        layout: Partial<WidgetConfig>[] | null
        updatedAt: string | null
      }>('/dashboard/layout')
      if (res && Array.isArray(res.layout) && res.layout.length > 0) {
        const parsed = parseSavedList(res.layout)
        serverLayoutData.value = parsed
        if (syncMode.value === 'server') {
          widgets.value = parsed
          saveLocalLayoutCache()
        }
        return parsed
      }
      serverLayoutData.value = null
      return null
    } catch {
      return null
    } finally {
      loadingServer.value = false
    }
  }

  /**
   * Verifica se a mensagem de escolha do layout inicial precisa aparecer
   */
  async function checkServerPrompt() {
    const remote = await fetchServerLayout()
    if (!promptDismissed.value && remote && remote.length > 0) {
      showServerPrompt.value = true
    }
  }

  /**
   * Chamado quando o usuário escolhe a preferência no modal de prompt de 1ª execução
   */
  function chooseInitialSyncMode(mode: SyncMode) {
    promptDismissed.value = true
    localStorage.setItem(PROMPT_DISMISSED_KEY, 'true')
    setSyncMode(mode)
    showServerPrompt.value = false
  }

  /**
   * Altera o modo de sincronização (Servidor x Local) nas configurações ou no dashboard
   */
  function setSyncMode(mode: SyncMode) {
    syncMode.value = mode
    localStorage.setItem(SYNC_MODE_KEY, mode)
    promptDismissed.value = true
    localStorage.setItem(PROMPT_DISMISSED_KEY, 'true')

    if (mode === 'server' && serverLayoutData.value) {
      widgets.value = serverLayoutData.value
      saveLocalLayoutCache()
    } else if (mode === 'local') {
      widgets.value = loadInitialLayout()
    }
  }

  /**
   * Envia o layout atual do dashboard para o servidor e avisa todos via SSE
   */
  async function saveLayoutGlobally(): Promise<boolean> {
    savingGlobal.value = true
    try {
      const exportable = getExportableList(widgets.value)

      await apiService.post('/dashboard/layout', {
        layout: exportable,
        clientId: clientIdSession.value,
      })
      serverLayoutData.value = [...widgets.value]
      setSyncMode('server')
      return true
    } catch {
      return false
    } finally {
      savingGlobal.value = false
    }
  }

  /**
   * Atualização em tempo real via SSE disparada por outro dispositivo
   */
  function applyRealtimeServerLayout(
    remoteLayout: Partial<WidgetConfig>[],
    remoteClientId?: string | null
  ) {
    if (!Array.isArray(remoteLayout)) return
    const parsed = parseSavedList(remoteLayout)
    serverLayoutData.value = parsed

    if (remoteClientId && remoteClientId === clientIdSession.value) {
      if (syncMode.value === 'server') {
        widgets.value = parsed
        saveLocalLayoutCache()
      }
      return
    }

    if (promptDismissed.value) {
      if (syncMode.value === 'server') {
        widgets.value = parsed
        saveLocalLayoutCache()
      }
      return
    }

    showServerPrompt.value = true
  }

  function toggleEditMode(val?: boolean) {
    isEditMode.value = typeof val === 'boolean' ? val : !isEditMode.value
  }

  function toggleWidgetVisibility(id: string, visible?: boolean) {
    const item = widgets.value.find((w) => w.id === id)
    if (item) {
      item.visible = typeof visible === 'boolean' ? visible : !item.visible
      saveLocalLayoutCache()
      if (syncMode.value === 'server') {
        void saveLayoutGlobally()
      }
    }
  }

  function removeWidget(id: string) {
    const index = widgets.value.findIndex((w) => w.id === id)
    if (index !== -1) {
      if (isDefaultWidget(id)) {
        widgets.value[index].visible = false
      } else {
        widgets.value.splice(index, 1)
      }
      saveLocalLayoutCache()
      if (syncMode.value === 'server') {
        void saveLayoutGlobally()
      }
    }
  }

  function addCustomWidget(
    templateType: string,
    customConfig: WidgetCustomConfig,
    customTitle?: string
  ): WidgetConfig {
    const entry = catalogEntry(templateType)
    const tmpl = entry ? fromCatalog(entry) : null
    const cols = tmpl ?? sized('half')
    const id = `${templateType}_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`

    const newWidget: WidgetConfig = {
      id,
      type: templateType,
      title: customTitle || (tmpl ? tmpl.title : 'Card Personalizado'),
      category: tmpl ? tmpl.category : 'charts',
      cols: cols.cols,
      sm: cols.sm,
      md: cols.md,
      lg: cols.lg,
      visible: true,
      order: widgets.value.length,
      description: tmpl ? tmpl.description : '',
      icon: tmpl ? tmpl.icon : 'mdi-view-dashboard-customize',
      config: customConfig,
    }

    widgets.value.push(newWidget)
    saveLocalLayoutCache()
    if (syncMode.value === 'server') {
      void saveLayoutGlobally()
    }

    return newWidget
  }

  function updateWidgetConfig(
    id: string,
    updates: { title?: string; config?: WidgetCustomConfig }
  ) {
    const item = widgets.value.find((w) => w.id === id)
    if (item) {
      if (updates.title) item.title = updates.title
      if (updates.config) item.config = { ...(item.config || {}), ...updates.config }
      saveLocalLayoutCache()
      if (syncMode.value === 'server') {
        void saveLayoutGlobally()
      }
    }
  }

  function resizeWidget(id: string, size: WidgetSize) {
    const item = widgets.value.find((w) => w.id === id)
    if (!item) return
    Object.assign(item, sized(size))
    saveLocalLayoutCache()
    if (syncMode.value === 'server') {
      void saveLayoutGlobally()
    }
  }

  function moveWidget(id: string, direction: 'up' | 'down') {
    const visibleList = visibleWidgets.value
    const currentIndex = visibleList.findIndex((w) => w.id === id)
    if (currentIndex === -1) return

    const targetIndex = direction === 'up' ? currentIndex - 1 : currentIndex + 1
    if (targetIndex < 0 || targetIndex >= visibleList.length) return

    const currentItem = visibleList[currentIndex]
    const targetItem = visibleList[targetIndex]

    const tempOrder = currentItem.order
    currentItem.order = targetItem.order
    targetItem.order = tempOrder

    widgets.value.sort((a, b) => a.order - b.order)
    widgets.value.forEach((w, idx) => {
      w.order = idx
    })

    saveLocalLayoutCache()
    if (syncMode.value === 'server') {
      void saveLayoutGlobally()
    }
  }

  function reorderWidgets(newOrderIds: string[]) {
    newOrderIds.forEach((id, index) => {
      const item = widgets.value.find((w) => w.id === id)
      if (item) {
        item.order = index
      }
    })
    widgets.value.sort((a, b) => a.order - b.order)
    saveLocalLayoutCache()
    if (syncMode.value === 'server') {
      void saveLayoutGlobally()
    }
  }

  function resetToDefaultLayout() {
    widgets.value = defaultWidgets()
    saveLocalLayoutCache()
    if (syncMode.value === 'server') {
      void saveLayoutGlobally()
    }
  }

  return {
    catalog,
    ready,
    loadCatalog,
    cardTemplates,
    isDefaultWidget,
    areaOfWidget,
    availableWidgets,
    isEditMode,
    syncMode,
    promptDismissed,
    showServerPrompt,
    serverLayoutData,
    savingGlobal,
    loadingServer,
    widgets,
    sortedWidgets,
    visibleWidgets,
    hiddenWidgets,
    fetchServerLayout,
    checkServerPrompt,
    chooseInitialSyncMode,
    setSyncMode,
    saveLayoutGlobally,
    applyRealtimeServerLayout,
    toggleEditMode,
    toggleWidgetVisibility,
    removeWidget,
    addCustomWidget,
    updateWidgetConfig,
    resizeWidget,
    moveWidget,
    reorderWidgets,
    resetToDefaultLayout,
  }
})
