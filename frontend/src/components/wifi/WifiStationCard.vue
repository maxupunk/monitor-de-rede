<template>
  <div class="d-flex flex-column ga-3">
    <div class="d-flex align-start justify-space-between ga-2">
      <WifiStationIdentity :row="row" class="flex-grow-1" />
      <WifiHealthChip :health="row.health" class="flex-shrink-0" />
    </div>

    <v-alert
      v-if="row.healthReason"
      :color="HEALTH[row.health].color"
      variant="tonal"
      density="compact"
      :icon="false"
      class="text-body-2 font-weight-medium"
    >
      {{ row.healthReason }}
    </v-alert>

    <div class="metrics">
      <div>
        <div class="metric-label">Sinal</div>
        <WifiStationSignal :row="row" />
      </div>
      <div>
        <div class="metric-label">Velocidade</div>
        <WifiStationRates :row="row" class="text-body-2" />
      </div>
      <div>
        <div class="metric-label">Latência</div>
        <WifiPingResult :row="row" placeholder />
      </div>
    </div>

    <div>
      <WifiStationNetwork :row="row" :show-device="showDevice" />
      <div v-if="details" class="text-caption mt-1">{{ details }}</div>
    </div>

    <div v-if="auth.canWrite" class="d-flex ga-2">
      <WifiPingButton :row="row" block class="flex-1-1-0" />
      <WifiWatchButton :row="row" block class="flex-1-1-0" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { HEALTH, stationDetails, type WifiStationRow } from '@/utils/wifiPresentation'
import WifiHealthChip from './WifiHealthChip.vue'
import WifiPingButton from './WifiPingButton.vue'
import WifiPingResult from './WifiPingResult.vue'
import WifiStationIdentity from './WifiStationIdentity.vue'
import WifiStationNetwork from './WifiStationNetwork.vue'
import WifiStationRates from './WifiStationRates.vue'
import WifiStationSignal from './WifiStationSignal.vue'
import WifiWatchButton from './WifiWatchButton.vue'

const props = defineProps<{ row: WifiStationRow; showDevice?: boolean }>()

const auth = useAuthStore()
const details = computed(() => stationDetails(props.row))
</script>

<style scoped>
/* Os números lado a lado; em tela estreita, a latência desce sozinha. */
.metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
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
