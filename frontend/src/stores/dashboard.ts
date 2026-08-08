import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'

export type WidgetCategory = 'summary' | 'lists' | 'charts'
export type SyncMode = 'server' | 'local'

export interface WidgetConfig {
  id: string
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
}

const STORAGE_KEY = 'netmonitor_dashboard_layout_v1'
const SYNC_MODE_KEY = 'netmonitor_dashboard_sync_mode'
const PROMPT_DISMISSED_KEY = 'netmonitor_dashboard_prompt_dismissed'

export const DEFAULT_WIDGETS: WidgetConfig[] = [
  {
    id: 'stat_cards',
    title: 'Cards de Resumo Estatístico',
    category: 'summary',
    cols: 12,
    sm: 12,
    md: 12,
    lg: 12,
    visible: true,
    order: 0,
    icon: 'mdi-view-dashboard-outline',
    description: 'Visão sintetizada de Dispositivos, Monitores, Disponibilidade e Alertas.',
  },
  {
    id: 'health_gauge',
    title: 'Saúde Global & Status dos Ativos',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 1,
    icon: 'mdi-gauge',
    description:
      'Gráfico Donut com taxa global de disponibilidade e distribuição dos status dos monitores.',
  },
  {
    id: 'latency_time_series',
    title: 'Latência & Perda de Pacotes',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 2,
    icon: 'mdi-chart-timeline-variant',
    description: 'Série temporal estilo Grafana com filtro de tempo (5m, 15m, 1h, 24h).',
  },
  {
    id: 'active_alerts',
    title: 'Alertas Críticos Ativos',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 3,
    icon: 'mdi-bell-outline',
    description: 'Lista dos alertas ativos com severidade crítica e ações de gerenciamento.',
  },
  {
    id: 'events_feed',
    title: 'Feed de Eventos Realtime',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 4,
    icon: 'mdi-pulse',
    description: 'Fluxo em tempo real dos eventos e mudanças de status via SSE.',
  },
  {
    id: 'network_monitors',
    title: 'Monitores de Rede',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 12,
    lg: 12,
    visible: true,
    order: 5,
    icon: 'mdi-chart-timeline-variant',
    description: 'Lista interativa de monitores com barras de histórico e scroll suave (420px).',
  },
  {
    id: 'event_distribution',
    title: 'Distribuição de Eventos por Hora',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 6,
    icon: 'mdi-chart-bar',
    description: 'Histograma por hora agrupando eventos por severidade (Crítico, Alerta, Info).',
  },
  {
    id: 'dns_latency',
    title: 'Latência e Benchmark de DNS',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 7,
    icon: 'mdi-dns-outline',
    description: 'Ranking comparativo e histórico de performance de resolvedores DNS.',
  },
]

export const useDashboardStore = defineStore('dashboard', () => {
  const isEditMode = ref(false)
  const syncMode = ref<SyncMode>((localStorage.getItem(SYNC_MODE_KEY) as SyncMode) || 'server')
  const promptDismissed = ref(localStorage.getItem(PROMPT_DISMISSED_KEY) === 'true')
  const showServerPrompt = ref(false)
  const clientIdSession = ref('client-' + Math.random().toString(36).substring(2, 10))

  const serverLayoutData = ref<WidgetConfig[] | null>(null)
  const savingGlobal = ref(false)
  const loadingServer = ref(false)

  const widgets = ref<WidgetConfig[]>(loadInitialLayout())

  function parseSavedList(savedList: Partial<WidgetConfig>[]): WidgetConfig[] {
    const mapSaved = new Map(savedList.map((item) => [item.id, item]))
    const merged: WidgetConfig[] = []

    savedList.forEach((savedItem, index) => {
      const def = DEFAULT_WIDGETS.find((w) => w.id === savedItem.id)
      if (def) {
        merged.push({
          ...def,
          visible: savedItem.visible ?? def.visible,
          order: typeof savedItem.order === 'number' ? savedItem.order : index,
        })
      }
    })

    DEFAULT_WIDGETS.forEach((def) => {
      if (!mapSaved.has(def.id)) {
        merged.push({ ...def, order: merged.length })
      }
    })

    merged.sort((a, b) => a.order - b.order)
    return merged
  }

  function loadInitialLayout(): WidgetConfig[] {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      if (!raw) return DEFAULT_WIDGETS.map((w) => ({ ...w }))
      const savedList = JSON.parse(raw) as Partial<WidgetConfig>[]
      if (!Array.isArray(savedList)) return DEFAULT_WIDGETS.map((w) => ({ ...w }))
      return parseSavedList(savedList)
    } catch {
      return DEFAULT_WIDGETS.map((w) => ({ ...w }))
    }
  }

  function saveLocalLayoutCache() {
    try {
      const exportable = widgets.value.map((w) => ({
        id: w.id,
        visible: w.visible,
        order: w.order,
      }))
      localStorage.setItem(STORAGE_KEY, JSON.stringify(exportable))
    } catch {
      // Ignora erro no localStorage
    }
  }

  const sortedWidgets = computed(() => {
    return [...widgets.value].sort((a, b) => a.order - b.order)
  })

  const visibleWidgets = computed(() => {
    return sortedWidgets.value.filter((w) => w.visible)
  })

  const hiddenWidgets = computed(() => {
    return sortedWidgets.value.filter((w) => !w.visible)
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
      const exportable = widgets.value.map((w) => ({
        id: w.id,
        visible: w.visible,
        order: w.order,
      }))

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

    // Se a alteração veio deste próprio navegador/aba, atualiza se estiver em modo servidor
    if (remoteClientId && remoteClientId === clientIdSession.value) {
      if (syncMode.value === 'server') {
        widgets.value = parsed
        saveLocalLayoutCache()
      }
      return
    }

    // Se o usuário já respondeu a preferência neste navegador (1x), respeita o modo escolhido sem perguntar de novo
    if (promptDismissed.value) {
      if (syncMode.value === 'server') {
        widgets.value = parsed
        saveLocalLayoutCache()
      }
      return
    }

    // Apenas se NUNCA respondeu a pergunta neste navegador, abre o prompt
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
    widgets.value = DEFAULT_WIDGETS.map((w) => ({ ...w }))
    saveLocalLayoutCache()
    if (syncMode.value === 'server') {
      void saveLayoutGlobally()
    }
  }

  return {
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
    moveWidget,
    reorderWidgets,
    resetToDefaultLayout,
  }
})
