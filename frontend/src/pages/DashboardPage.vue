<template>
  <div class="dashboard-page">
    <PageHeader title="Dashboard" :subtitle="headerSubtitle">
      <template #actions>
        <v-btn
          color="secondary"
          variant="tonal"
          prepend-icon="mdi-robot-outline"
          :disabled="aiStore.isStreaming"
          @click="askAiToAdjust"
        >
          <span class="hidden-sm-and-down">Ajustar com IA</span>
          <span class="hidden-md-and-up">IA</span>
        </v-btn>
        <v-btn
          :color="dashboardStore.isEditMode ? 'success' : 'primary'"
          :variant="dashboardStore.isEditMode ? 'flat' : 'tonal'"
          :prepend-icon="dashboardStore.isEditMode ? 'mdi-check' : 'mdi-pencil-ruler'"
          @click="dashboardStore.toggleEditMode()"
        >
          {{ dashboardStore.isEditMode ? 'Concluir' : 'Personalizar' }}
        </v-btn>
        <v-btn
          icon
          variant="tonal"
          color="primary"
          :loading="loading"
          aria-label="Recarregar dados"
          @click="refreshData"
        >
          <v-icon>mdi-refresh</v-icon>
          <v-tooltip activator="parent" location="bottom">Recarregar dados</v-tooltip>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Barra do modo de personalização -->
    <v-sheet v-if="dashboardStore.isEditMode" rounded="lg" class="edit-bar dashboard-surface mb-4">
      <div class="d-flex align-center ga-3">
        <v-icon color="primary">mdi-gesture-tap-hold</v-icon>
        <div>
          <div class="text-subtitle-2 font-weight-bold">Personalizando o Dashboard</div>
          <div class="text-body-2">
            Arraste os cards para reordenar; use o menu de cada card para mudar a largura ou
            ocultar.
          </div>
        </div>
      </div>

      <div class="d-flex align-center ga-2 flex-wrap">
        <v-btn-toggle
          :model-value="dashboardStore.syncMode"
          density="compact"
          variant="tonal"
          color="primary"
          divided
          mandatory
          rounded="lg"
          @update:model-value="(val: SyncMode) => dashboardStore.setSyncMode(val)"
        >
          <v-btn value="server" size="small" prepend-icon="mdi-cloud-sync">
            Todos os dispositivos
          </v-btn>
          <v-btn value="local" size="small" prepend-icon="mdi-laptop">Só este navegador</v-btn>
        </v-btn-toggle>
        <v-btn
          color="primary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-plus"
          @click="addWidgetDialog = true"
        >
          Adicionar widget
        </v-btn>
        <v-btn
          color="warning"
          variant="tonal"
          size="small"
          prepend-icon="mdi-restore"
          @click="confirmReset"
        >
          Restaurar padrão
        </v-btn>
      </div>
    </v-sheet>

    <!-- Grade de widgets: cada tipo vem do registro -->
    <v-row class="dashboard-grid">
      <v-col
        v-for="(widget, idx) in gridWidgets"
        :key="widget.id"
        :cols="widget.cols || 12"
        :sm="widget.sm || 12"
        :md="widget.md || 12"
        :lg="widget.lg || 12"
      >
        <DashboardWidgetWrapper
          :widget="widget"
          :is-edit-mode="dashboardStore.isEditMode"
          :is-first="idx === 0"
          :is-last="idx === gridWidgets.length - 1"
          @move-up="dashboardStore.moveWidget(widget.id, 'up')"
          @move-down="dashboardStore.moveWidget(widget.id, 'down')"
          @resize="(size: WidgetSize) => dashboardStore.resizeWidget(widget.id, size)"
          @remove="dashboardStore.removeWidget(widget.id)"
          @reorder="handleReorder"
        >
          <component :is="widgetComponentOf(widget)" v-bind="widgetPropsOf(widget)" />
        </DashboardWidgetWrapper>
      </v-col>
    </v-row>

    <div v-if="gridWidgets.length === 0" class="dashboard-empty dashboard-surface rounded-lg">
      <v-icon size="40" color="primary">mdi-view-dashboard-edit-outline</v-icon>
      <div class="text-subtitle-1 font-weight-bold mt-2">O Dashboard está vazio</div>
      <div class="text-body-2 mb-4">Adicione widgets ou volte ao layout padrão.</div>
      <div class="d-flex ga-2">
        <v-btn color="primary" prepend-icon="mdi-plus" @click="addWidgetDialog = true">
          Adicionar widget
        </v-btn>
        <v-btn color="primary" variant="tonal" @click="dashboardStore.resetToDefaultLayout()">
          Restaurar padrão
        </v-btn>
      </div>
    </div>

    <AddWidgetDialog v-model="addWidgetDialog" />

    <!-- Boas-vindas / escolha do layout do servidor (1x por navegador) -->
    <DashboardServerPromptDialog />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import {
  useDashboardStore,
  type SyncMode,
  type WidgetConfig,
  type WidgetSize,
} from '@/stores/dashboard'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useEventsStore } from '@/stores/events'
import { useMonitorsStore } from '@/stores/monitors'
import { useDockerStore } from '@/stores/docker'
import { useAiStore } from '@/stores/ai'
import { useConfirm } from '@/composables/useConfirm'
import DashboardWidgetWrapper from '@/components/DashboardWidgetWrapper.vue'
import AddWidgetDialog from '@/components/AddWidgetDialog.vue'
import DashboardServerPromptDialog from '@/components/DashboardServerPromptDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import {
  widgetComponentOf,
  widgetPropsOf,
  widgetTypeOf,
} from '@/components/dashboard/widgetRegistry'

const dashboardStore = useDashboardStore()
const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()
const monitorsStore = useMonitorsStore()
const dockerStore = useDockerStore()
const aiStore = useAiStore()
const { confirm } = useConfirm()
const loading = ref(false)
const addWidgetDialog = ref(false)

/**
 * Widgets que entram na grade: visíveis, de um tipo que esta versão conhece
 * e com a fonte de dados presente — o resumo do Docker some sem Engine.
 */
const gridWidgets = computed(() =>
  dashboardStore.visibleWidgets.filter((widget) => {
    if (!widgetComponentOf(widget)) return false
    if (widgetTypeOf(widget) === 'docker_summary') {
      return dockerStore.localView.status?.available === true || dashboardStore.isEditMode
    }
    return true
  })
)

const headerSubtitle = computed(() =>
  dashboardStore.syncMode === 'server'
    ? 'Visão geral em tempo real · layout compartilhado entre os dispositivos'
    : 'Visão geral em tempo real · layout só deste navegador'
)

function handleReorder(draggedId: string, targetId: string) {
  const currentVisible = dashboardStore.visibleWidgets.map((w) => w.id)
  const fromIndex = currentVisible.indexOf(draggedId)
  const toIndex = currentVisible.indexOf(targetId)
  if (fromIndex === -1 || toIndex === -1) return
  currentVisible.splice(fromIndex, 1)
  currentVisible.splice(toIndex, 0, draggedId)
  dashboardStore.reorderWidgets(currentVisible)
}

async function confirmReset() {
  const ok = await confirm({
    title: 'Restaurar o layout padrão?',
    message: 'Os cards personalizados serão removidos e a ordem volta à de fábrica.',
    confirmText: 'Restaurar',
    confirmColor: 'warning',
    icon: 'mdi-restore',
  })
  if (ok) dashboardStore.resetToDefaultLayout()
}

/**
 * A IA lê o layout atual e propõe mudanças; nada é aplicado sem o operador
 * confirmar no chat.
 */
function askAiToAdjust() {
  aiStore.askAbout(
    'Analise o meu Dashboard atual (widgets, ordem e larguras) junto com o que existe na rede ' +
      'monitorada e proponha ajustes para deixá-lo mais útil e organizado. Explique o porquê ' +
      'de cada mudança antes de aplicar.',
    {}
  )
}

let unbindLayout: (() => void) | null = null

onMounted(async () => {
  unbindLayout = eventsStore.onEvent('dashboard:layout_updated', (data) => {
    dashboardStore.applyRealtimeServerLayout(
      data.layout as Partial<WidgetConfig>[],
      data.clientId as string | null
    )
  })

  await dashboardStore.checkServerPrompt()
  await refreshData()
})

onUnmounted(() => {
  unbindLayout?.()
})

async function refreshData() {
  loading.value = true
  await Promise.all([
    devicesStore.fetchDevices(),
    alertsStore.fetchActiveAlerts(),
    monitorsStore.fetchMonitors(),
    // Qual host é o desta central (o agente local): o resumo Docker vem dele.
    dockerStore.fetchHosts(),
  ])
  loading.value = false
}
</script>

<style scoped>
.edit-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  padding: 14px 20px;
  border-left: 4px solid rgb(var(--v-theme-primary)) !important;
}

.dashboard-grid {
  margin-top: -4px;
}
</style>
