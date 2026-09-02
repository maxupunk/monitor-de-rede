import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'

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

export interface CardTemplate {
  type: string
  title: string
  category: WidgetCategory
  icon: string
  description: string
  compatibleResourceTypes: ResourceCompatibilityType[]
  allowMultiple: boolean
  defaultCols: { cols?: number; sm?: number; md?: number; lg?: number }
}

export const CARD_TEMPLATES: CardTemplate[] = [
  {
    type: 'ether_bandwidth',
    title: 'Consumo de Banda de Ether',
    category: 'charts',
    icon: 'mdi-swap-horizontal-bold',
    description:
      'Gráfico de tráfego de entrada e saída (Rx/Tx) para uma interface de rede específica.',
    compatibleResourceTypes: ['bandwidth'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 6, lg: 6 },
  },
  {
    type: 'bandwidth_vs_latency',
    title: 'Consumo de Banda vs Latência',
    category: 'charts',
    icon: 'mdi-chart-multiaxis',
    description:
      'Gráfico correlacionado com eixo duplo comparando tráfego de rede (Mbps) e latência de ping (ms).',
    compatibleResourceTypes: ['dual-axis', 'bandwidth', 'numeric'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 12, lg: 12 },
  },
  {
    type: 'cpu_usage',
    title: 'Uso de CPU',
    category: 'charts',
    icon: 'mdi-cpu-64-bit',
    description:
      'Monitoramento de utilização de CPU (%) com linha de tendência e alertas de limite.',
    compatibleResourceTypes: ['numeric'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 6, lg: 6 },
  },
  {
    type: 'ram_usage',
    title: 'Uso de RAM',
    category: 'charts',
    icon: 'mdi-memory',
    description: 'Monitoramento da quantidade de memória RAM usada e da capacidade disponível.',
    compatibleResourceTypes: ['numeric'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 6, lg: 6 },
  },
  {
    type: 'dns_latency',
    title: 'Tempo de Resolução DNS (Alinhado)',
    category: 'charts',
    icon: 'mdi-dns-outline',
    description:
      'Gráfico e ranking comparativo com escala de tempo de consulta unificada para resolvedores DNS.',
    compatibleResourceTypes: ['dns-resolvers', 'numeric'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 6, lg: 6 },
  },
  {
    type: 'binary_status',
    title: 'Status Binário (Up/Down)',
    category: 'lists',
    icon: 'mdi-checkbox-blank-circle-outline',
    description:
      'Card binário exclusivo para estados booleanos (Online/Offline, Link Up/Down, Check Pass/Fail).',
    compatibleResourceTypes: ['binary'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 6, lg: 6 },
  },
  {
    type: 'saas_heatmap',
    title: 'Mapa de Calor de Latência SaaS',
    category: 'charts',
    icon: 'mdi-chart-scatter-plot-hexbin',
    description:
      'Matriz horária (00h-23h) de latência por hora do dia e identificação de picos de lentidão.',
    compatibleResourceTypes: ['numeric'],
    allowMultiple: true,
    defaultCols: { cols: 12, sm: 12, md: 12, lg: 12 },
  },
  {
    type: 'saas_services',
    title: 'Serviços SaaS, Bancos & Nuvem',
    category: 'lists',
    icon: 'mdi-bank-check',
    description:
      'Painel de monitoramento de latência e qualidade de experiência (QoE) para bancos, fintechs e provedores SaaS.',
    compatibleResourceTypes: ['numeric'],
    allowMultiple: false,
    defaultCols: { cols: 12, sm: 12, md: 12, lg: 12 },
  },
]

const STORAGE_KEY = 'netmonitor_dashboard_layout_v1'
const SYNC_MODE_KEY = 'netmonitor_dashboard_sync_mode'
const PROMPT_DISMISSED_KEY = 'netmonitor_dashboard_prompt_dismissed'

export const DEFAULT_WIDGETS: WidgetConfig[] = [
  // Linha 1: Visão geral numérica — sempre no topo
  {
    id: 'stat_cards',
    type: 'stat_cards',
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
  // Linha 2: Saúde global (donut) + Alertas críticos ativos (lado a lado)
  {
    id: 'health_gauge',
    type: 'health_gauge',
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
    id: 'active_alerts',
    type: 'active_alerts',
    title: 'Alertas Críticos Ativos',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 2,
    icon: 'mdi-bell-outline',
    description: 'Lista dos alertas ativos com severidade crítica e ações de gerenciamento.',
  },
  // Linha 3: Alvos instáveis (oscilação) + Feed de eventos em tempo real
  {
    id: 'unstable_targets',
    type: 'unstable_targets',
    title: 'Alvos Instáveis',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 3,
    icon: 'mdi-sine-wave',
    description:
      'Ranking dos alvos que mais caíram e voltaram na janela — quem oscila não aparece na lista de alertas ativos.',
  },
  {
    id: 'events_feed',
    type: 'events_feed',
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
  // Linha 4: DNS — diagnóstico de resolução (largura total para melhor leitura)
  {
    id: 'dns_latency',
    type: 'dns_latency',
    title: 'Tempo de Consulta e Benchmark DNS',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 12,
    lg: 12,
    visible: true,
    order: 5,
    icon: 'mdi-dns-outline',
    description: 'Ranking comparativo e histórico de tempo de consulta de resolvedores DNS.',
  },
  // Linha 5: Lista completa de monitores (full-width para acomodar timeline)
  {
    id: 'network_monitors',
    type: 'network_monitors',
    title: 'Monitores de Rede',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 12,
    lg: 12,
    visible: true,
    order: 6,
    icon: 'mdi-chart-timeline-variant',
    description: 'Lista interativa de monitores com barras de histórico e scroll suave (420px).',
  },
  // Linha 6: Gráficos de série temporal — latência e distribuição de eventos
  {
    id: 'latency_time_series',
    type: 'latency_time_series',
    title: 'Latência & Perda de Pacotes',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 7,
    icon: 'mdi-chart-timeline-variant',
    description: 'Série temporal estilo Grafana com filtro de tempo (5m, 15m, 1h, 24h).',
  },
  {
    id: 'event_distribution',
    type: 'event_distribution',
    title: 'Distribuição de Eventos por Hora',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 6,
    lg: 6,
    visible: true,
    order: 8,
    icon: 'mdi-chart-bar',
    description: 'Histograma por hora agrupando eventos por severidade (Crítico, Alerta, Info).',
  },
  {
    id: 'saas_services',
    type: 'saas_services',
    title: 'Serviços SaaS, Bancos & Nuvem',
    category: 'lists',
    cols: 12,
    sm: 12,
    md: 12,
    lg: 12,
    visible: true,
    order: 9,
    icon: 'mdi-bank-check',
    description:
      'Painel de monitoramento de latência e qualidade de experiência (QoE) para bancos, fintechs e provedores SaaS.',
  },
  {
    id: 'saas_heatmap',
    type: 'saas_heatmap',
    title: 'Mapa de Calor de Latência SaaS',
    category: 'charts',
    cols: 12,
    sm: 12,
    md: 12,
    lg: 12,
    visible: true,
    order: 10,
    icon: 'mdi-chart-scatter-plot-hexbin',
    description:
      'Matriz horária (00h-23h) de latência por hora do dia e identificação de picos de lentidão.',
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
    const merged: WidgetConfig[] = []
    const processedIds = new Set<string>()

    savedList.forEach((savedItem, index) => {
      if (!savedItem.id) return

      // Procura primeiro em DEFAULT_WIDGETS
      const def = DEFAULT_WIDGETS.find((w) => w.id === savedItem.id)
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
        const tmpl = CARD_TEMPLATES.find((t) => t.type === savedItem.type)
        const baseCategory = tmpl ? tmpl.category : savedItem.category || 'charts'
        const baseIcon = tmpl ? tmpl.icon : savedItem.icon || 'mdi-view-dashboard-customize'
        const baseCols = tmpl ? tmpl.defaultCols : { cols: 12, sm: 12, md: 6, lg: 6 }

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

    // Adiciona widgets padrão que não estavam salvos no layout
    DEFAULT_WIDGETS.forEach((def) => {
      if (!processedIds.has(def.id)) {
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
      const isDefault = DEFAULT_WIDGETS.some((def) => def.id === id)
      if (isDefault) {
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
    const tmpl = CARD_TEMPLATES.find((t) => t.type === templateType)
    const id = `${templateType}_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`

    const newWidget: WidgetConfig = {
      id,
      type: templateType,
      title: customTitle || (tmpl ? tmpl.title : 'Card Personalizado'),
      category: tmpl ? tmpl.category : 'charts',
      cols: tmpl?.defaultCols.cols ?? 12,
      sm: tmpl?.defaultCols.sm ?? 12,
      md: tmpl?.defaultCols.md ?? 6,
      lg: tmpl?.defaultCols.lg ?? 6,
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
    removeWidget,
    addCustomWidget,
    updateWidgetConfig,
    moveWidget,
    reorderWidgets,
    resetToDefaultLayout,
  }
})
