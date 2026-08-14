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
            @update:model-value="(val) => dashboardStore.setSyncMode(val as any)"
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
          <v-row v-if="widget.id === 'stat_cards'">
            <v-col cols="12" sm="6" md="3">
              <v-card
                elevation="2"
                class="pa-4 rounded-lg stat-card"
                :to="statCardLink('/devices')"
              >
                <div class="d-flex align-center justify-space-between mb-2">
                  <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold"
                  >Dispositivos</span
                  >
                  <v-avatar color="primary" variant="tonal" size="36">
                    <v-icon color="primary">mdi-devices</v-icon>
                  </v-avatar>
                </div>
                <div class="text-h4 font-weight-bold">{{ devicesStore.totalCount }}</div>
                <div class="text-caption text-success font-weight-medium mt-1">
                  {{ devicesStore.onlineCount }} online / {{ devicesStore.offlineCount }} offline
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6" md="3">
              <v-card
                elevation="2"
                class="pa-4 rounded-lg stat-card"
                :to="statCardLink('/monitors')"
              >
                <div class="d-flex align-center justify-space-between mb-2">
                  <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold"
                  >Monitores de Rede</span
                  >
                  <v-avatar color="info" variant="tonal" size="36">
                    <v-icon color="info">mdi-chart-timeline-variant</v-icon>
                  </v-avatar>
                </div>
                <div class="text-h4 font-weight-bold">{{ monitorsStore.monitors.length }}</div>
                <div class="text-caption text-info font-weight-medium mt-1">
                  {{ healthCounts.up }} operacionais
                  <template v-if="healthCounts.disabled > 0">
                    · {{ healthCounts.disabled }} desativado(s)
                  </template>
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6" md="3">
              <v-card
                elevation="2"
                class="pa-4 rounded-lg stat-card"
                :to="statCardLink({ path: '/monitors', query: { status: 'down' } })"
              >
                <div class="d-flex align-center justify-space-between mb-2">
                  <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold"
                  >Disponibilidade</span
                  >
                  <v-avatar color="success" variant="tonal" size="36">
                    <v-icon color="success">mdi-check-circle-outline</v-icon>
                  </v-avatar>
                </div>
                <div class="text-h4 font-weight-bold text-success">{{ globalUptime }}%</div>
                <div class="text-caption text-grey mt-1">
                  {{ healthCounts.up }} de {{ healthCounts.monitored }} monitor(es) ativo(s) no ar
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6" md="3">
              <v-card elevation="2" class="pa-4 rounded-lg stat-card" :to="statCardLink('/alerts')">
                <div class="d-flex align-center justify-space-between mb-2">
                  <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold"
                  >Alertas Ativos</span
                  >
                  <v-avatar color="warning" variant="tonal" size="36">
                    <v-icon color="warning">mdi-bell-ring-outline</v-icon>
                  </v-avatar>
                </div>
                <div class="text-h4 font-weight-bold text-warning">
                  {{ alertsStore.activeAlerts.length }}
                </div>
                <div class="text-caption text-warning mt-1">Requerem atenção</div>
              </v-card>
            </v-col>
          </v-row>

          <!-- 2. Saúde Global (Gauge/Donut) -->
          <GaugeHealthWidget v-else-if="widget.id === 'health_gauge'" />

          <!-- 3. Latência & Perda de Pacotes (Time Series) -->
          <LatencyTimeSeriesWidget v-else-if="widget.id === 'latency_time_series'" />

          <!-- 4. Alertas Críticos Ativos -->
          <v-card
            v-else-if="widget.id === 'active_alerts'"
            elevation="2"
            class="rounded-lg fill-height"
          >
            <v-card-title
              class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2"
            >
              <div class="d-flex align-center">
                <v-icon start color="warning">mdi-bell-outline</v-icon>
                <span class="font-weight-bold text-h6">Alertas Críticos Ativos</span>
                <v-chip size="x-small" color="warning" class="ml-2" variant="tonal">
                  {{ alertsStore.activeAlerts.length }}
                </v-chip>
              </div>
              <v-btn
                v-if="alertsStore.activeAlerts.length > 0"
                size="small"
                color="primary"
                variant="outlined"
                prepend-icon="mdi-refresh"
                :loading="verifyingAll"
                @click="handleVerifyAllAlerts"
              >
                Verificar Todos
              </v-btn>
            </v-card-title>
            <v-divider></v-divider>
            <v-card-text class="pa-0">
              <div v-if="alertsStore.activeAlerts.length > 0">
                <v-list lines="two">
                  <v-list-item
                    v-for="alert in alertsStore.activeAlerts.slice(0, 5)"
                    :key="alert.id"
                    :title="alert.title"
                    :subtitle="alert.message"
                    class="px-4 py-2 border-b cursor-pointer"
                    @click="goToAlert(alert)"
                  >
                    <template #prepend>
                      <v-avatar
                        :color="
                          alert.severity === 'critical' || alert.severity === 'error'
                            ? 'error'
                            : 'warning'
                        "
                        size="36"
                      >
                        <v-icon color="white">mdi-alert</v-icon>
                      </v-avatar>
                    </template>
                    <template #append>
                      <div class="d-flex flex-column flex-md-row align-end align-md-center ga-2">
                        <v-chip variant="outlined" size="x-small" class="font-weight-medium">
                          {{ statusLabel(alert.status) }}
                        </v-chip>
                        <div class="d-flex ga-1">
                          <v-tooltip text="Verificar se resolveu">
                            <template #activator="{ props: tooltipProps }">
                              <v-btn
                                v-bind="tooltipProps"
                                icon
                                size="small"
                                variant="text"
                                color="info"
                                :loading="verifyingId === alert.id"
                                @click.stop="handleVerifyAlert(alert.id)"
                              >
                                <v-icon>mdi-refresh</v-icon>
                              </v-btn>
                            </template>
                          </v-tooltip>
                          <v-tooltip text="Reconhecer alerta (testa e remove se resolvido)">
                            <template #activator="{ props: tooltipProps }">
                              <v-btn
                                v-bind="tooltipProps"
                                icon
                                size="small"
                                variant="text"
                                color="primary"
                                :disabled="alert.status === 'acknowledged'"
                                :loading="verifyingId === alert.id"
                                @click.stop="handleAcknowledgeAlert(alert.id)"
                              >
                                <v-icon>mdi-check-circle-outline</v-icon>
                              </v-btn>
                            </template>
                          </v-tooltip>
                          <v-tooltip text="Silenciar alerta">
                            <template #activator="{ props: tooltipProps }">
                              <v-btn
                                v-bind="tooltipProps"
                                icon
                                size="small"
                                variant="text"
                                color="warning"
                                :disabled="alert.status === 'silenced'"
                                @click.stop="openSilenceDialog(alert.id)"
                              >
                                <v-icon>mdi-bell-off-outline</v-icon>
                              </v-btn>
                            </template>
                          </v-tooltip>
                        </div>
                      </div>
                    </template>
                  </v-list-item>
                </v-list>
              </div>
              <div v-else class="pa-6 text-center text-grey">
                <v-icon size="44" color="success" class="mb-2">mdi-check-all</v-icon>
                <div class="text-subtitle-2 font-weight-medium">Nenhum alerta ativo!</div>
                <div class="text-caption">Todos os sistemas estão funcionando normalmente.</div>
              </div>
            </v-card-text>
          </v-card>

          <!-- 5. Feed de Eventos em Tempo Real -->
          <v-card
            v-else-if="widget.id === 'events_feed'"
            elevation="2"
            class="rounded-lg fill-height"
          >
            <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
              <div class="d-flex align-center">
                <v-icon start color="info">mdi-pulse</v-icon>
                <span class="font-weight-bold text-h6">Feed de Eventos em Tempo Real</span>
              </div>
              <v-chip
                :color="eventsStore.isConnected ? 'success' : 'error'"
                size="x-small"
                variant="flat"
              >
                {{ eventsStore.isConnected ? 'Ao Vivo' : 'Desconectado' }}
              </v-chip>
            </v-card-title>
            <v-divider></v-divider>
            <v-card-text class="pa-0">
              <div v-if="eventsStore.recentEvents.length > 0">
                <v-list max-height="360" class="overflow-y-auto pa-0">
                  <v-list-item
                    v-for="(evt, evtIdx) in eventsStore.recentEvents.slice(0, 10)"
                    :key="evtIdx"
                    :title="formatEventDetails(evt).title"
                    :subtitle="formatEventDetails(evt).message"
                    class="px-4 py-2 border-b cursor-pointer"
                    @click="openEventDetail(evt)"
                  >
                    <template #prepend>
                      <v-avatar
                        :color="formatEventDetails(evt).color"
                        size="32"
                        variant="tonal"
                        class="mr-3"
                      >
                        <v-icon size="18">{{ formatEventDetails(evt).icon }}</v-icon>
                      </v-avatar>
                    </template>
                    <template #append>
                      <span class="text-caption text-grey">{{ formatEventDetails(evt).time }}</span>
                    </template>
                  </v-list-item>
                </v-list>
              </div>
              <div v-else class="pa-6 text-center text-grey">
                <v-icon size="44" color="grey-lighten-1" class="mb-2">
                  mdi-access-point-network
                </v-icon>
                <div class="text-subtitle-2 font-weight-medium">
                  Aguardando eventos em tempo real...
                </div>
                <div class="text-caption">
                  Mudanças de status e verificações aparecerão aqui automaticamente.
                </div>
              </div>
            </v-card-text>
          </v-card>

          <!-- 6. Monitores de Rede com Scroll Limito (420px) -->
          <v-card v-else-if="widget.id === 'network_monitors'" elevation="2" class="rounded-lg">
            <v-card-title
              class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2"
            >
              <div class="d-flex align-center">
                <v-icon start color="primary">mdi-chart-timeline-variant</v-icon>
                <span class="font-weight-bold text-h6">Monitores de Rede</span>
                <v-chip size="x-small" color="primary" class="ml-2" variant="tonal">
                  {{ monitorsStore.monitors.length }}
                </v-chip>
              </div>
              <v-btn
                variant="text"
                color="primary"
                size="small"
                append-icon="mdi-arrow-right"
                to="/monitors"
              >
                Ver Todos os Monitores
              </v-btn>
            </v-card-title>
            <v-divider></v-divider>

            <v-card-text class="pa-0">
              <div v-if="monitorsStore.monitors.length > 0">
                <v-list class="monitors-scroll-container pa-0">
                  <v-list-item
                    v-for="monitor in monitorsStore.monitors"
                    :key="monitor.id"
                    class="px-4 py-3 border-b"
                  >
                    <div
                      class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-3 w-100"
                    >
                      <div
                        class="monitor-info d-flex align-center ga-3"
                        style="min-width: 220px; flex: 1"
                      >
                        <v-avatar
                          :color="getStatusColor(monitor.status)"
                          size="10"
                          class="mr-1"
                        ></v-avatar>
                        <div>
                          <router-link
                            :to="'/monitors/' + monitor.id"
                            class="text-subtitle-1 font-weight-bold text-decoration-none text-primary hover-underline d-block"
                          >
                            {{ monitor.name }}
                          </router-link>

                          <div class="d-flex align-center ga-2 mt-1">
                            <v-chip size="x-small" color="info" variant="tonal">
                              {{ (monitor.type || 'N/A').toUpperCase() }}
                            </v-chip>
                            <span class="text-caption text-grey-darken-1">{{
                              monitor.target
                            }}</span>
                          </div>
                        </div>
                      </div>

                      <div
                        class="monitor-timeline d-flex align-center justify-start justify-md-center monitor-timeline-scroll"
                        style="flex: 2; min-width: 280px"
                      >
                        <router-link
                          :to="'/monitors/' + monitor.id"
                          class="text-decoration-none d-flex align-center ga-2"
                        >
                          <template v-if="isGaugeMonitor(monitor)">
                            <MonitorSparkline
                              :data="monitor.gaugeHistory || []"
                              :color="gaugeSparklineColor(monitor)"
                              :width="189"
                              :height="28"
                              :unit="isTrafficMonitor(monitor) ? 'bps' : '%'"
                            />
                            <span
                              class="text-caption font-weight-medium text-high-emphasis text-no-wrap"
                            >
                              {{ formatGaugeShortValue(monitor) }}
                            </span>
                          </template>
                          <MonitorTimelineBar
                            v-else
                            :results="monitor.recentResults"
                            :max-blocks="24"
                            :height="20"
                            :width="5"
                          ></MonitorTimelineBar>
                        </router-link>
                      </div>

                      <div
                        class="monitor-actions d-flex align-center ga-2 justify-start justify-md-end"
                        style="min-width: 140px"
                      >
                        <v-chip
                          :color="getStatusColor(monitor.status)"
                          size="small"
                          variant="tonal"
                          class="font-weight-medium"
                        >
                          {{ (monitor.status || 'UNKNOWN').toUpperCase() }}
                        </v-chip>
                        <v-btn
                          size="small"
                          color="primary"
                          variant="outlined"
                          prepend-icon="mdi-play"
                          :loading="monitorsStore.runningId === monitor.id"
                          @click="monitorsStore.runMonitor(monitor.id)"
                        >
                          Testar
                        </v-btn>
                      </div>
                    </div>
                  </v-list-item>
                </v-list>
              </div>
              <div v-else class="pa-8 text-center text-grey">
                <v-icon size="48" color="grey-lighten-1" class="mb-2">
                  mdi-chart-timeline-variant-off
                </v-icon>
                <div class="text-subtitle-1 font-weight-medium">Nenhum monitor cadastrado</div>
                <p class="text-caption text-grey-darken-1 mb-4">
                  Cadastre monitores ICMP, HTTP ou TCP para visualizar os gráficos em barras.
                </p>
                <v-btn color="primary" prepend-icon="mdi-plus" to="/monitors">
                  Cadastrar Monitor
                </v-btn>
              </div>
            </v-card-text>
          </v-card>

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

    <v-snackbar v-model="snackbar" :color="snackbarColor" location="bottom right" timeout="4000">
      {{ snackbarText }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter, type RouteLocationRaw } from 'vue-router'
import { useDashboardStore } from '@/stores/dashboard'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { useMonitorsStore } from '@/stores/monitors'
import DashboardWidgetWrapper from '@/components/DashboardWidgetWrapper.vue'
import AddWidgetDialog from '@/components/AddWidgetDialog.vue'
import DashboardServerPromptDialog from '@/components/DashboardServerPromptDialog.vue'
import GaugeHealthWidget from '@/components/widgets/GaugeHealthWidget.vue'
import LatencyTimeSeriesWidget from '@/components/widgets/LatencyTimeSeriesWidget.vue'
import EventDistributionWidget from '@/components/widgets/EventDistributionWidget.vue'
import EtherBandwidthWidget from '@/components/widgets/EtherBandwidthWidget.vue'
import BandwidthVsLatencyWidget from '@/components/widgets/BandwidthVsLatencyWidget.vue'
import CpuUsageWidget from '@/components/widgets/CpuUsageWidget.vue'
import RamUsageWidget from '@/components/widgets/RamUsageWidget.vue'
import BinaryStatusWidget from '@/components/widgets/BinaryStatusWidget.vue'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import DnsLatencyCard from '@/components/DnsLatencyCard.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import EventDetailDialog from '@/components/EventDetailDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import { statusLabel } from '@/utils/alertPresentation'
import { formatEventDetails } from '@/utils/eventPresentation'
import { formatBps } from '@/utils/formatters'
import {
  getStatusColor,
  isGaugeMonitor,
  isTrafficMonitor,
  gaugeMetricName,
  gaugeHexColor,
  monitorHealthCounts,
  monitorUptimePercent,
} from '@/utils/monitorPresentation'
import type { Monitor } from '@/stores/monitors'

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

function goToAlert(alert: { monitorId?: number | null; deviceId?: number | null }) {
  router.push(getAlertLink(alert))
}

function openEventDetail(evt: RealtimeEventPayload) {
  selectedEventPayload.value = evt
  eventDetailDialog.value = true
}

function getAlertLink(alert: { monitorId?: number | null; deviceId?: number | null }): string {
  if (alert.monitorId) return '/monitors/' + alert.monitorId
  if (alert.deviceId) return '/devices/' + alert.deviceId
  return '/alerts'
}

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
    dashboardStore.applyRealtimeServerLayout(data.layout as any, data.clientId as string | null)
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

function gaugeSparklineColor(monitor: Monitor): string {
  return gaugeHexColor(monitor.gaugeMetric?.value ?? null, gaugeMetricName(monitor))
}

function formatGaugeShortValue(item: Monitor): string {
  if (!item.gaugeMetric) return 'N/D'
  if (isTrafficMonitor(item)) {
    return formatBps(item.gaugeMetric.value, { fractionDigits: 1 })
  }
  return `${Math.round(item.gaugeMetric.value)}%`
}

/**
 * Cada card de resumo abre a tela que responde por aquele número. Em modo de
 * edição o link some: ali o clique é para arrastar e reordenar o painel, não
 * para sair do dashboard.
 */
function statCardLink(target: string | RouteLocationRaw): RouteLocationRaw | undefined {
  return dashboardStore.isEditMode ? undefined : target
}

const healthCounts = computed(() => monitorHealthCounts(monitorsStore.monitors))

const globalUptime = computed(() => monitorUptimePercent(healthCounts.value))
</script>

<style scoped>
.hover-underline:hover {
  text-decoration: underline !important;
}

.edit-banner {
  border: 2px dashed rgba(var(--v-theme-info), 0.6);
}

/* O card leva a uma tela: precisa reagir ao ponteiro para parecer clicável. */
.stat-card {
  transition:
    transform 0.15s ease,
    box-shadow 0.15s ease;
}

.stat-card:hover {
  transform: translateY(-2px);
}

.monitors-scroll-container {
  max-height: 420px;
  overflow-y: auto;
}

.monitors-scroll-container::-webkit-scrollbar {
  width: 6px;
}

.monitors-scroll-container::-webkit-scrollbar-thumb {
  background: rgba(148, 163, 184, 0.4);
  border-radius: 4px;
}

.monitors-scroll-container::-webkit-scrollbar-thumb:hover {
  background: rgba(148, 163, 184, 0.7);
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
