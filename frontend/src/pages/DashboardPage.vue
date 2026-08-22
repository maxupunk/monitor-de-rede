<template>
  <div>
    <PageHeader title="Dashboard" subtitle="Visão geral do monitoramento e status em tempo real">
      <template #actions>
        <v-chip
          :color="eventsStore.isConnected ? 'success' : 'warning'"
          variant="tonal"
          size="small"
          class="font-weight-medium"
        >
          <v-icon start size="12" :color="eventsStore.isConnected ? 'success' : 'warning'">
            mdi-circle
          </v-icon>
          <span class="hidden-xs">{{
            eventsStore.isConnected ? 'SSE Conectado' : 'SSE Reconectando...'
          }}</span>
          <span class="hidden-sm-and-up">{{ eventsStore.isConnected ? 'SSE' : '...' }}</span>
        </v-chip>

        <v-chip
          :color="dashboardStore.syncMode === 'server' ? 'info' : 'grey'"
          variant="tonal"
          size="small"
          class="font-weight-medium cursor-pointer"
          to="/settings"
        >
          <v-icon start size="14">
            {{ dashboardStore.syncMode === 'server' ? 'mdi-cloud-sync' : 'mdi-laptop' }}
          </v-icon>
          <span class="hidden-xs">{{
            dashboardStore.syncMode === 'server' ? 'Servidor (Global)' : 'Modo Local'
          }}</span>
          <span class="hidden-sm-and-up">{{
            dashboardStore.syncMode === 'server' ? 'Global' : 'Local'
          }}</span>
          <v-tooltip activator="parent" location="bottom">
            {{
              dashboardStore.syncMode === 'server'
                ? 'Sincronizado em tempo real via SSE com todos os dispositivos'
                : 'Personalizado apenas para este navegador/dispositivo'
            }}
          </v-tooltip>
        </v-chip>

        <v-btn
          :color="dashboardStore.isEditMode ? 'warning' : 'secondary'"
          variant="tonal"
          prepend-icon="mdi-pencil-ruler"
          @click="dashboardStore.toggleEditMode()"
        >
          <span class="hidden-sm-and-down">{{
            dashboardStore.isEditMode ? 'Sair da Edição' : 'Editar Dashboard'
          }}</span>
          <span class="hidden-md-and-up">{{ dashboardStore.isEditMode ? 'Sair' : 'Editar' }}</span>
        </v-btn>

        <v-btn color="primary" prepend-icon="mdi-refresh" :loading="loading" @click="refreshData">
          <span class="hidden-sm-and-down">Atualizar Dados</span>
          <span class="hidden-md-and-up">Atualizar</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Banner do Modo de Edição (Estilo Grafana & Sincronização) -->
    <v-alert
      v-if="dashboardStore.isEditMode"
      type="info"
      variant="tonal"
      class="mb-6 rounded-lg edit-banner border-dashed"
    >
      <div class="d-flex flex-column flex-sm-row align-center justify-space-between ga-3">
        <div class="d-flex align-center ga-2">
          <v-icon color="info">mdi-tune-vertical</v-icon>
          <div>
            <div class="text-subtitle-2 font-weight-bold">Modo de Edição Ativo</div>
            <div class="text-caption">
              Arraste os cards, altere a ordem ou oculte painéis para customizar sua experiência.
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-2 flex-wrap justify-end">
          <v-btn-toggle
            :model-value="dashboardStore.syncMode"
            density="compact"
            variant="outlined"
            divided
            mandatory
            @update:model-value="(val) => dashboardStore.setSyncMode(val as SyncMode)"
          >
            <v-btn value="server" size="x-small">
              <v-icon start size="14">mdi-cloud-sync</v-icon>
              Servidor
            </v-btn>
            <v-btn value="local" size="x-small">
              <v-icon start size="14">mdi-laptop</v-icon>
              Local
            </v-btn>
          </v-btn-toggle>

          <v-btn
            color="info"
            variant="flat"
            size="small"
            prepend-icon="mdi-cloud-upload"
            :loading="dashboardStore.savingGlobal"
            @click="dashboardStore.saveLayoutGlobally()"
          >
            <span class="hidden-xs">Salvar para Todos</span>
            <span class="hidden-sm-and-up">Salvar Todos</span>
            <v-tooltip activator="parent" location="top">
              Salva este layout no servidor para sincronizar em tempo real com todos os dispositivos
            </v-tooltip>
          </v-btn>

          <v-btn
            color="secondary"
            variant="tonal"
            size="small"
            prepend-icon="mdi-plus"
            @click="addWidgetDialog = true"
          >
            Adicionar Widget
          </v-btn>

          <v-btn
            color="warning"
            variant="outlined"
            size="small"
            prepend-icon="mdi-restore"
            @click="dashboardStore.resetToDefaultLayout()"
          >
            Restaurar Padrão
          </v-btn>

          <v-btn
            color="success"
            variant="flat"
            size="small"
            prepend-icon="mdi-check"
            @click="dashboardStore.toggleEditMode(false)"
          >
            Concluir
          </v-btn>
        </div>
      </div>
    </v-alert>

    <!-- Malha Reativa de Widgets Customizáveis -->
    <v-row class="mb-2">
      <v-col
        v-for="(widget, idx) in dashboardStore.visibleWidgets"
        :key="widget.id"
        :cols="widget.cols || 12"
        :sm="widget.sm || 12"
        :md="widget.md || 12"
        :lg="widget.lg || 12"
        class="pb-6"
      >
        <DashboardWidgetWrapper
          :widget="widget"
          :is-edit-mode="dashboardStore.isEditMode"
          :is-first="idx === 0"
          :is-last="idx === dashboardStore.visibleWidgets.length - 1"
          @move-up="dashboardStore.moveWidget(widget.id, 'up')"
          @move-down="dashboardStore.moveWidget(widget.id, 'down')"
          @remove="dashboardStore.removeWidget(widget.id)"
          @reorder="handleReorder"
        >
          <!-- 1. Cards de Resumo Estatístico -->
          <StatCardsWidget v-if="widget.id === 'stat_cards'" />

          <!-- 2. Saúde Global (Gauge/Donut) -->
          <GaugeHealthWidget v-else-if="widget.id === 'health_gauge'" />

          <!-- 3. Latência & Perda de Pacotes (Time Series) -->
          <LatencyTimeSeriesWidget v-else-if="widget.id === 'latency_time_series'" />

          <!-- 4. Alertas Críticos Ativos -->
          <ActiveAlertsWidget
            v-else-if="widget.id === 'active_alerts'"
            :verifying-id="verifyingId"
            :verifying-all="verifyingAll"
            @verify="handleVerifyAlert"
            @acknowledge="handleAcknowledgeAlert"
            @silence="openSilenceDialog"
            @verify-all="handleVerifyAllAlerts"
            @go-to-alert="goToAlert"
          />

          <!-- 5. Feed de Eventos em Tempo Real -->
          <EventsFeedWidget
            v-else-if="widget.id === 'events_feed'"
            @open-detail="openEventDetail"
          />

          <!-- 6. Monitores de Rede com Scroll Limito (420px) -->
          <NetworkMonitorsWidget v-else-if="widget.id === 'network_monitors'" />

          <!-- 7. Distribuição de Eventos por Hora -->
          <EventDistributionWidget
            v-else-if="widget.id === 'event_distribution' || widget.type === 'event_distribution'"
          />

          <!-- 8. Latência DNS -->
          <DnsLatencyCard
            v-else-if="widget.id === 'dns_latency' || widget.type === 'dns_latency'"
          />

          <!-- 9. Consumo de Banda de Ether -->
          <EtherBandwidthWidget
            v-else-if="widget.type === 'ether_bandwidth' || widget.id === 'ether_bandwidth'"
            :widget="widget"
          />

          <!-- 10. Consumo de Banda vs Latência -->
          <BandwidthVsLatencyWidget
            v-else-if="
              widget.type === 'bandwidth_vs_latency' || widget.id === 'bandwidth_vs_latency'
            "
            :widget="widget"
          />

          <!-- 11. Uso de CPU -->
          <CpuUsageWidget
            v-else-if="widget.type === 'cpu_usage' || widget.id === 'cpu_usage'"
            :widget="widget"
          />

          <!-- 12. Uso de RAM -->
          <RamUsageWidget
            v-else-if="widget.type === 'ram_usage' || widget.id === 'ram_usage'"
            :widget="widget"
          />

          <!-- 13. Status Binário -->
          <BinaryStatusWidget
            v-else-if="widget.type === 'binary_status' || widget.id === 'binary_status'"
            :widget="widget"
          />

          <!-- 14. Alvos Instáveis (oscilação por alvo na janela) -->
          <UnstableTargetsWidget
            v-else-if="widget.type === 'unstable_targets' || widget.id === 'unstable_targets'"
          />

          <!-- 15. Mapa de Calor de Latência SaaS -->
          <SaasLatencyHeatmapWidget
            v-else-if="widget.type === 'saas_heatmap' || widget.id === 'saas_heatmap'"
            :monitor-id="
              typeof widget.config?.monitorId === 'number' ? widget.config.monitorId : undefined
            "
          />

          <!-- 16. Serviços SaaS, Bancos & Nuvem -->
          <SaasServicesWidget
            v-else-if="widget.type === 'saas_services' || widget.id === 'saas_services'"
          />
        </DashboardWidgetWrapper>
      </v-col>
    </v-row>

    <!-- Modal Silenciar Alerta -->
    <AlertSilenceDialog v-model="silenceDialog" :alert-id="silenceTargetId" />

    <!-- Modal Detalhes do Evento -->
    <EventDetailDialog v-model="eventDetailDialog" :event="selectedEventPayload" />

    <!-- Modal Catálogo de Widgets -->
    <AddWidgetDialog v-model="addWidgetDialog" />

    <!-- Modal Boas-vindas / Prompt de Escolha do Servidor (Exibido 1x por navegador) -->
    <DashboardServerPromptDialog />

    <MonitorDetailDialog v-model="monitorDetailDialog" :monitor-id="monitorDetailId" />

    <v-snackbar v-model="snackbar" :color="snackbarColor" location="bottom right" timeout="4000">
      {{ snackbarText }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useDashboardStore, type SyncMode, type WidgetConfig } from '@/stores/dashboard'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { useMonitorsStore } from '@/stores/monitors'
import DashboardWidgetWrapper from '@/components/DashboardWidgetWrapper.vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import AddWidgetDialog from '@/components/AddWidgetDialog.vue'
import DashboardServerPromptDialog from '@/components/DashboardServerPromptDialog.vue'
import StatCardsWidget from '@/components/dashboard/StatCardsWidget.vue'
import ActiveAlertsWidget from '@/components/dashboard/ActiveAlertsWidget.vue'
import EventsFeedWidget from '@/components/dashboard/EventsFeedWidget.vue'
import NetworkMonitorsWidget from '@/components/dashboard/NetworkMonitorsWidget.vue'
import GaugeHealthWidget from '@/components/widgets/GaugeHealthWidget.vue'
import LatencyTimeSeriesWidget from '@/components/widgets/LatencyTimeSeriesWidget.vue'
import EventDistributionWidget from '@/components/widgets/EventDistributionWidget.vue'
import EtherBandwidthWidget from '@/components/widgets/EtherBandwidthWidget.vue'
import BandwidthVsLatencyWidget from '@/components/widgets/BandwidthVsLatencyWidget.vue'
import CpuUsageWidget from '@/components/widgets/CpuUsageWidget.vue'
import RamUsageWidget from '@/components/widgets/RamUsageWidget.vue'
import BinaryStatusWidget from '@/components/widgets/BinaryStatusWidget.vue'
import UnstableTargetsWidget from '@/components/widgets/UnstableTargetsWidget.vue'
import SaasLatencyHeatmapWidget from '@/components/widgets/SaasLatencyHeatmapWidget.vue'
import SaasServicesWidget from '@/components/widgets/SaasServicesWidget.vue'
import DnsLatencyCard from '@/components/DnsLatencyCard.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import EventDetailDialog from '@/components/EventDetailDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import type { AlertEvent } from '@/stores/alerts'

const router = useRouter()
const dashboardStore = useDashboardStore()
const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()
const monitorsStore = useMonitorsStore()
const loading = ref(false)

const addWidgetDialog = ref(false)
const silenceDialog = ref(false)
const silenceTargetId = ref<number | null>(null)

const eventDetailDialog = ref(false)
const selectedEventPayload = ref<RealtimeEventPayload | null>(null)

const verifyingId = ref<number | null>(null)
const verifyingAll = ref(false)
const snackbar = ref(false)
const snackbarText = ref('')
const snackbarColor = ref('success')

async function handleAcknowledgeAlert(id: number) {
  verifyingId.value = id
  const result = await alertsStore.acknowledgeAlert(id)
  verifyingId.value = null

  if (result.resolved) {
    snackbarText.value = `Alerta #${id} verificado e resolvido automaticamente!`
    snackbarColor.value = 'success'
    snackbar.value = true
  } else if (result.success) {
    snackbarText.value = `Alerta #${id} reconhecido (continua em falha).`
    snackbarColor.value = 'info'
    snackbar.value = true
  } else {
    snackbarText.value = result.message || 'Erro ao reconhecer alerta.'
    snackbarColor.value = 'error'
    snackbar.value = true
  }
}

async function handleVerifyAlert(id: number) {
  verifyingId.value = id
  const result = await alertsStore.verifyAlert(id)
  verifyingId.value = null

  if (result.resolved) {
    snackbarText.value = `Alerta #${id} verificado e resolvido!`
    snackbarColor.value = 'success'
    snackbar.value = true
  } else if (result.success) {
    snackbarText.value = `Alerta #${id} re-verificado: continua com falha.`
    snackbarColor.value = 'warning'
    snackbar.value = true
  } else {
    snackbarText.value = result.message || 'Erro ao verificar alerta.'
    snackbarColor.value = 'error'
    snackbar.value = true
  }
}

async function handleVerifyAllAlerts() {
  verifyingAll.value = true
  const result = await alertsStore.verifyAllAlerts()
  verifyingAll.value = false

  if (result.success) {
    snackbarText.value = result.message || 'Verificação concluída.'
    snackbarColor.value = result.resolvedCount && result.resolvedCount > 0 ? 'success' : 'info'
    snackbar.value = true
  } else {
    snackbarText.value = result.message || 'Erro ao verificar alertas.'
    snackbarColor.value = 'error'
    snackbar.value = true
  }
}

function openSilenceDialog(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

/**
 * Leva o operador ao alvo do alerta.
 *
 * Monitor abre em diálogo — é a única forma de abrir `/monitors/{id}` no
 * produto; dispositivo continua sendo navegação de página, porque ali o
 * contexto **é** a página.
 */
function goToAlert(alert: AlertEvent) {
  if (alert.monitorId) {
    abrirMonitor(alert.monitorId)
    return
  }
  router.push(alert.deviceId ? `/devices/${alert.deviceId}` : '/alerts')
}

function openEventDetail(evt: RealtimeEventPayload) {
  selectedEventPayload.value = evt
  eventDetailDialog.value = true
}

const {
  detalheAberto: monitorDetailDialog,
  monitorEmDetalhe: monitorDetailId,
  abrirDetalhe: abrirMonitor,
} = useMonitorDetail()

function handleReorder(draggedId: string, targetId: string) {
  const currentVisible = dashboardStore.visibleWidgets.map((w) => w.id)
  const fromIndex = currentVisible.indexOf(draggedId)
  const toIndex = currentVisible.indexOf(targetId)

  if (fromIndex !== -1 && toIndex !== -1) {
    currentVisible.splice(fromIndex, 1)
    currentVisible.splice(toIndex, 0, draggedId)
    dashboardStore.reorderWidgets(currentVisible)
  }
}

onMounted(async () => {
  eventsStore.onEvent('dashboard:layout_updated', (data) => {
    dashboardStore.applyRealtimeServerLayout(
      data.layout as Partial<WidgetConfig>[],
      data.clientId as string | null
    )
  })

  await dashboardStore.checkServerPrompt()
  await refreshData()
})

async function refreshData() {
  loading.value = true
  await Promise.all([
    devicesStore.fetchDevices(),
    alertsStore.fetchActiveAlerts(),
    monitorsStore.fetchMonitors(),
  ])
  loading.value = false
}
</script>

<style scoped>
.edit-banner {
  border: 2px dashed rgba(var(--v-theme-info), 0.6);
}

.ga-1 {
  gap: 4px;
}
.ga-2 {
  gap: 8px;
}
.ga-3 {
  gap: 12px;
}
.ga-4 {
  gap: 16px;
}
</style>
