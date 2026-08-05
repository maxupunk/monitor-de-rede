<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6 flex-wrap ga-3">
      <div>
        <h1 class="text-h4 font-weight-bold">Feed de Eventos em Tempo Real</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Fluxo contínuo de eventos transmitidos via Server-Sent Events (SSE)
        </p>
      </div>
      <v-chip :color="eventsStore.isConnected ? 'success' : 'grey'" size="large" variant="tonal">
        <v-icon start>mdi-radiobox-marked</v-icon>
        {{ eventsStore.isConnected ? 'SSE Conectado' : 'SSE Desconectado' }}
      </v-chip>
    </div>

    <!-- Filtros e Busca -->
    <v-card elevation="2" class="rounded-lg mb-6 pa-4">
      <v-row density="compact">
        <v-col cols="12" sm="8" md="6">
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
        <v-col cols="12" sm="4" md="4">
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
        <v-col cols="12" md="2" class="d-flex align-center justify-end">
          <v-chip variant="outlined" size="small" color="primary">
            {{ filteredEvents.length }} de {{ eventsStore.recentEvents.length }} eventos
          </v-chip>
        </v-col>
      </v-row>
    </v-card>

    <!-- Feed de Eventos -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 font-weight-bold d-flex align-center justify-space-between">
        <span>Transmissão em Tempo Real</span>
        <span class="text-caption text-grey font-weight-regular">
          Clique no evento para visualizar o payload detalhado
        </span>
      </v-card-title>
      <v-divider />
      <v-card-text class="pa-0">
        <v-list lines="two" class="pa-0">
          <v-list-item
            v-for="(evt, idx) in filteredEvents"
            :key="idx"
            class="px-4 py-3 border-b cursor-pointer"
            @click="openEventDetail(evt)"
          >
            <template #prepend>
              <v-avatar
                :color="formatEventDetails(evt).color"
                variant="tonal"
                size="36"
                class="mr-3"
              >
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
        <div v-if="filteredEvents.length === 0" class="pa-8 text-center text-grey">
          <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-filter-remove-outline</v-icon>
          <div class="text-subtitle-2 font-weight-medium">Nenhum evento encontrado</div>
          <div class="text-caption">
            Tente ajustar os termos de busca ou remover o filtro selecionado.
          </div>
        </div>
      </v-card-text>
    </v-card>

    <!-- Modal Detalhes do Evento -->
    <EventDetailDialog v-model="detailDialog" :event="selectedEvent" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { formatEventDetails } from '@/utils/eventPresentation'
import EventDetailDialog from '@/components/EventDetailDialog.vue'

const eventsStore = useEventsStore()

const searchQuery = ref('')
const typeFilter = ref('all')
const detailDialog = ref(false)
const selectedEvent = ref<RealtimeEventPayload | null>(null)

const typeOptions = [
  { title: 'Todos os tipos', value: 'all' },
  { title: 'Métricas Coletadas (metric:recorded)', value: 'metric:recorded' },
  { title: 'Resultado de Monitor (monitor:result)', value: 'monitor:result' },
  { title: 'Status de Dispositivo (device:status)', value: 'device:status' },
  { title: 'Alertas (alert:*)', value: 'alerts' },
  { title: 'Interfaces (interface:*)', value: 'interfaces' },
  { title: 'Status de Probe (probe:status)', value: 'probe:status' },
  { title: 'Varredura (discovery:*)', value: 'discovery' },
]

const filteredEvents = computed(() => {
  return eventsStore.recentEvents.filter((evt) => {
    // 1. Filtro por tipo
    if (typeFilter.value !== 'all') {
      if (typeFilter.value === 'alerts' && !evt.type.startsWith('alert:')) return false
      if (typeFilter.value === 'interfaces' && !evt.type.startsWith('interface:')) return false
      if (typeFilter.value === 'discovery' && !evt.type.startsWith('discovery:')) return false
      if (
        !typeFilter.value.includes('*') &&
        evt.type !== typeFilter.value &&
        !typeFilter.value.endsWith(':*')
      ) {
        if (
          typeFilter.value !== 'alerts' &&
          typeFilter.value !== 'interfaces' &&
          typeFilter.value !== 'discovery'
        ) {
          return false
        }
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
