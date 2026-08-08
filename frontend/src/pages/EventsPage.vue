<template>
  <div>
    <PageHeader
      title="Feed de Eventos em Tempo Real"
      subtitle="Fluxo contínuo de eventos transmitidos via Server-Sent Events (SSE)"
    >
      <template #actions>
        <v-chip :color="eventsStore.isConnected ? 'success' : 'grey'" size="large" variant="tonal">
          <v-icon start>mdi-radiobox-marked</v-icon>
          <span class="hidden-xs">{{
            eventsStore.isConnected ? 'SSE Conectado' : 'SSE Desconectado'
          }}</span>
          <span class="hidden-sm-and-up">{{ eventsStore.isConnected ? 'On' : 'Off' }}</span>
        </v-chip>
      </template>
    </PageHeader>

    <!-- Filtros e Busca -->
    <v-card elevation="2" class="rounded-lg mb-6 pa-4">
      <v-row density="compact">
        <v-col cols="12" md="6">
          <v-text-field
            v-model="searchQuery"
            placeholder="Buscar por dispositivo, mensagem, métrica ou conteúdo JSON..."
            prepend-inner-icon="mdi-magnify"
            hide-details
            clearable
            density="compact"
            variant="outlined"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6" md="4">
          <v-select
            v-model="typeFilter"
            :items="typeOptions"
            item-title="title"
            item-value="value"
            label="Tipo de Evento"
            hide-details
            density="compact"
            variant="outlined"
          ></v-select>
        </v-col>
        <v-col cols="12" sm="6" md="2" class="d-flex align-center justify-start justify-md-end">
          <v-chip variant="outlined" size="small" color="primary">
            {{ allFilteredEvents.length }} de {{ combinedEvents.length }} eventos
          </v-chip>
        </v-col>
      </v-row>
    </v-card>

    <!-- Feed de Eventos -->
    <v-infinite-scroll :key="historicalEvents.scrollKey.value" @load="historicalEvents.load">
      <v-list lines="two" class="pa-0 rounded-lg border">
        <v-list-item
          v-for="(evt, idx) in allFilteredEvents"
          :key="idx"
          class="px-4 py-3 border-b cursor-pointer"
          @click="openEventDetail(evt)"
        >
          <template #prepend>
            <v-avatar :color="formatEventDetails(evt).color" variant="tonal" size="36" class="mr-3">
              <v-icon size="20">{{ formatEventDetails(evt).icon }}</v-icon>
            </v-avatar>
          </template>
          <v-list-item-title class="font-weight-bold">
            {{ formatEventDetails(evt).title }}
            <v-chip size="x-small" class="ml-2 font-weight-medium" variant="outlined">
              {{ evt.type }}
            </v-chip>
          </v-list-item-title>
          <v-list-item-subtitle class="text-body-2 mt-1">
            {{ formatEventDetails(evt).message }}
          </v-list-item-subtitle>
          <template #append>
            <span class="text-caption text-grey">{{ formatEventDetails(evt).time }}</span>
          </template>
        </v-list-item>
      </v-list>
      <template #empty>
        <div class="text-caption text-grey text-center py-4">
          Nenhum outro evento registrado no histórico.
        </div>
      </template>
    </v-infinite-scroll>
    <div v-if="allFilteredEvents.length === 0" class="pa-8 text-center text-grey">
      <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-filter-remove-outline</v-icon>
      <div class="text-subtitle-2 font-weight-medium">Nenhum evento encontrado</div>
      <div class="text-caption">
        Tente ajustar os termos de busca ou remover o filtro selecionado.
      </div>
    </div>

    <!-- Modal Detalhes do Evento -->
    <EventDetailDialog v-model="detailDialog" :event="selectedEvent" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { useInfiniteList } from '@/composables/useInfiniteList'
import { formatEventDetails } from '@/utils/eventPresentation'
import EventDetailDialog from '@/components/EventDetailDialog.vue'
import PageHeader from '@/components/PageHeader.vue'

interface HistoricalEventRecord {
  id: number
  alertRuleId?: number | null
  deviceId?: number | null
  monitorId?: number | null
  status: string
  severity: string
  message: string
  createdAt: string
  device?: { id: number; name: string }
  monitor?: { id: number; name: string }
}

const eventsStore = useEventsStore()

const searchQuery = ref('')
const typeFilter = ref('all')
const detailDialog = ref(false)
const selectedEvent = ref<RealtimeEventPayload | null>(null)

const historicalEvents = useInfiniteList<HistoricalEventRecord>(() => '/events', {
  label: 'eventos históricos',
})

/** Alertas persistidos entram no feed no mesmo formato dos eventos em tempo real */
const historicalPayloads = computed<RealtimeEventPayload[]>(() =>
  historicalEvents.items.value.map((evt) => ({
    type: evt.status === 'active' ? 'alert:opened' : 'alert:resolved',
    timestamp: evt.createdAt || new Date().toISOString(),
    data: {
      id: evt.id,
      deviceId: evt.deviceId,
      deviceName: evt.device?.name,
      monitorId: evt.monitorId,
      monitorName: evt.monitor?.name,
      severity: evt.severity,
      message: evt.message,
      status: evt.status,
    },
  }))
)

const typeOptions = [
  { title: 'Todos os tipos', value: 'all' },
  { title: 'Métricas Coletadas (metric:recorded)', value: 'metric:recorded' },
  { title: 'Resultado de Monitor (monitor:result)', value: 'monitor:result' },
  { title: 'Status de Dispositivo (device:status)', value: 'device:status' },
  { title: 'Alertas (alert:*)', value: 'alerts' },
  { title: 'Interfaces (interface:*)', value: 'interfaces' },
  { title: 'Status de Probe (probe:status)', value: 'probe:status' },
  { title: 'Varredura (discovery:*)', value: 'discovery' },
  { title: 'Túneis VPN (vpn:*)', value: 'vpn' },
]

const combinedEvents = computed(() => {
  const map = new Map<string, RealtimeEventPayload>()
  for (const evt of eventsStore.recentEvents) {
    const key = `${evt.type}-${evt.timestamp}-${JSON.stringify(evt.data)}`
    map.set(key, evt)
  }
  for (const evt of historicalPayloads.value) {
    const key = `${evt.type}-${evt.timestamp}-${JSON.stringify(evt.data)}`
    if (!map.has(key)) {
      map.set(key, evt)
    }
  }
  return Array.from(map.values())
})

const allFilteredEvents = computed(() => {
  return combinedEvents.value.filter((evt) => {
    // 1. Filtro por tipo
    if (typeFilter.value !== 'all') {
      // Filtros de família agrupam vários tipos sob um prefixo; os demais
      // comparam o tipo exato.
      const families: Record<string, string> = {
        alerts: 'alert:',
        interfaces: 'interface:',
        discovery: 'discovery:',
        vpn: 'vpn:',
      }
      const prefix = families[typeFilter.value]
      if (prefix) {
        if (!evt.type.startsWith(prefix)) return false
      } else if (evt.type !== typeFilter.value) {
        return false
      }
    }

    // 2. Filtro por texto
    if (!searchQuery.value.trim()) return true
    const query = searchQuery.value.toLowerCase()
    const formatted = formatEventDetails(evt)
    const jsonStr = formatted.rawJson.toLowerCase()

    return (
      evt.type.toLowerCase().includes(query) ||
      formatted.title.toLowerCase().includes(query) ||
      formatted.message.toLowerCase().includes(query) ||
      jsonStr.includes(query)
    )
  })
})

function openEventDetail(evt: RealtimeEventPayload) {
  selectedEvent.value = evt
  detailDialog.value = true
}
</script>
