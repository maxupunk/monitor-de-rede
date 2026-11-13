<template>
  <v-card border flat rounded="lg" class="panel" aria-label="Detalhes do aparelho">
    <v-card-item>
      <template #prepend>
        <v-avatar :color="HEALTH[row.health].color" variant="tonal" size="44">
          <v-icon size="26">{{ DEVICE_KIND[kind].icon }}</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-wrap">{{ name }}</v-card-title>
      <v-card-subtitle class="opacity-100"
        >{{ DEVICE_KIND[kind].label }} · {{ row.mac }}</v-card-subtitle
      >
      <template #append>
        <v-btn
          icon="mdi-close"
          variant="text"
          density="comfortable"
          aria-label="Fechar o painel"
          @click="emit('close')"
        ></v-btn>
      </template>
    </v-card-item>

    <v-card-text class="d-flex flex-column ga-4">
      <WifiStationHealth :row="row" />

      <div class="metrics">
        <div>
          <div class="metric-label">Sinal</div>
          <WifiStationSignal :row="row" />
        </div>
        <div>
          <div class="metric-label">Ruído</div>
          <span class="font-weight-bold">{{
            row.noiseDbm === null ? '—' : `${row.noiseDbm} dBm`
          }}</span>
        </div>
        <div>
          <div class="metric-label">Velocidade</div>
          <WifiStationRates :row="row" class="text-body-2" />
        </div>
        <div>
          <div class="metric-label">Retransmissões</div>
          <span :class="retriesClass">{{ formatPercent(row.txRetriesPct) }}</span>
        </div>
        <div>
          <div class="metric-label">Distância</div>
          <span class="font-weight-bold">{{ distanceRange }}</span>
          <div v-if="row.distance && !row.distance.calibrated" class="text-caption">
            sem a potência do rádio: menos confiável
          </div>
        </div>
        <div>
          <div class="metric-label">Conectado há</div>
          <span class="font-weight-bold">{{ connectedFor }}</span>
        </div>
      </div>

      <WifiStationIdentity :row="row" />
      <WifiStationNetwork :row="row" show-device />
      <div v-if="row.txPowerDbm !== null || row.frequencyMhz !== null" class="text-caption">
        Rádio do AP: {{ radioText }}
      </div>

      <div v-if="auth.canWrite" class="d-flex flex-wrap ga-2">
        <WifiPingButton :row="row" class="flex-1-1-0" />
        <WifiWatchButton :row="row" class="flex-1-1-0" />
      </div>
      <WifiPingResult v-if="row.canPing" :row="row" />

      <v-divider></v-divider>
      <WifiRoamingHistory :mac="row.mac" :retention-days="retentionDays" />
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { formatDistanceRange, formatPercent } from '@/utils/formatters'
import { formatOutputValue } from '@/utils/pluginPresentation'
import { DEVICE_KIND, deviceKind } from '@/utils/wifiMap/deviceKind'
import { GRADE_COLOR, HEALTH, stationName, type WifiStationRow } from '@/utils/wifiPresentation'
import WifiPingButton from './WifiPingButton.vue'
import WifiPingResult from './WifiPingResult.vue'
import WifiRoamingHistory from './WifiRoamingHistory.vue'
import WifiStationHealth from './WifiStationHealth.vue'
import WifiStationIdentity from './WifiStationIdentity.vue'
import WifiStationNetwork from './WifiStationNetwork.vue'
import WifiStationRates from './WifiStationRates.vue'
import WifiStationSignal from './WifiStationSignal.vue'
import WifiWatchButton from './WifiWatchButton.vue'

const props = defineProps<{ row: WifiStationRow; retentionDays?: number }>()
const emit = defineEmits<{ close: [] }>()

const auth = useAuthStore()
const store = useWifiClientsStore()

const kind = computed(() => deviceKind(props.row))
const name = computed(() => store.watchedByMac.get(props.row.mac)?.name ?? stationName(props.row))
const distanceRange = computed(() =>
  props.row.distance
    ? formatDistanceRange(props.row.distance.minM, props.row.distance.maxM)
    : 'sem estimativa'
)
const connectedFor = computed(() =>
  props.row.connectedS === null ? '—' : formatOutputValue(props.row.connectedS, 'uptime')
)
const retriesClass = computed(() =>
  props.row.retriesGrade
    ? `font-weight-bold text-${GRADE_COLOR[props.row.retriesGrade]}`
    : 'font-weight-bold'
)
const radioText = computed(() => {
  const parts: string[] = []
  if (props.row.txPowerDbm !== null) parts.push(`potência ${props.row.txPowerDbm} dBm`)
  if (props.row.frequencyMhz !== null) parts.push(`${props.row.frequencyMhz} MHz`)
  return parts.join(' · ')
})
</script>

<style scoped>
.metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
  gap: 12px;
}

.metric-label {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 4px;
}
</style>
