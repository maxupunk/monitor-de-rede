<template>
  <v-card rounded="lg" border>
    <v-card-item>
      <v-card-title class="d-flex align-center ga-2">
        <v-icon color="primary">mdi-wifi</v-icon>
        Clientes Wi-Fi
      </v-card-title>
      <v-card-subtitle class="text-wrap">
        <WifiReadSummary :snapshot="snapshot" />
      </v-card-subtitle>
      <template #append>
        <WifiRefreshButton
          :reading="snapshot?.status === 'reading'"
          @refresh="store.refresh(deviceId)"
        />
      </template>
    </v-card-item>

    <v-card-text class="d-flex flex-column ga-4">
      <v-alert v-if="store.error" type="error" variant="tonal" density="compact">
        {{ store.error }}
      </v-alert>
      <v-alert v-if="snapshot?.status === 'failed'" type="error" variant="tonal" density="compact">
        A última leitura falhou: {{ snapshot.error }}
      </v-alert>

      <WifiStationFilters v-model="filter" :counts="counts" />
      <WifiStationsTable
        :rows="filtered"
        :loading="!snapshot && store.requesting"
        :no-data-text="emptyText"
      />
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { countStations, emptyFilter, filterStations, stationRows } from '@/utils/wifiPresentation'
import WifiReadSummary from './WifiReadSummary.vue'
import WifiRefreshButton from './WifiRefreshButton.vue'
import WifiStationFilters from './WifiStationFilters.vue'
import WifiStationsTable from './WifiStationsTable.vue'

const props = defineProps<{ deviceId: number }>()

const store = useWifiClientsStore()
const auth = useAuthStore()
const filter = ref(emptyFilter())

const snapshot = computed(() => store.snapshotFor(props.deviceId))
const rows = computed(() => (snapshot.value ? stationRows([snapshot.value]) : []))
const counts = computed(() => countStations(rows.value))
const filtered = computed(() => filterStations(rows.value, filter.value))

const emptyText = computed(() => {
  if (!snapshot.value) return 'Ainda não houve leitura deste roteador.'
  if (rows.value.length === 0) return 'Nenhum cliente conectado agora.'
  return 'Nenhum cliente com esses filtros.'
})

// Primeira vez na aba sem leitura guardada: uma leitura, pedida uma vez — o
// resto chega pelo SSE.
onMounted(() => {
  if (!snapshot.value && auth.canWrite) store.refresh(props.deviceId)
})
</script>
