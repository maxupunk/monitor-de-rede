<template>
  <v-chip
    v-if="state?.result"
    size="small"
    variant="flat"
    :color="GRADE_COLOR[state.result.grade]"
    class="font-weight-bold"
  >
    {{ resultText }}
  </v-chip>
  <v-tooltip v-else-if="state?.error" :text="state.error" location="top" open-on-click>
    <template #activator="{ props: tip }">
      <v-chip
        v-bind="tip"
        size="small"
        variant="flat"
        color="error"
        prepend-icon="mdi-alert-circle-outline"
      >
        Falhou
      </v-chip>
    </template>
  </v-tooltip>
  <v-tooltip v-else-if="unavailable" :text="unavailable" location="top" open-on-click>
    <template #activator="{ props: tip }">
      <span v-bind="tip" class="d-inline-flex align-center ga-1 font-weight-medium">
        <v-icon size="16" color="info">mdi-information-outline</v-icon>
        Indisponível
      </span>
    </template>
  </v-tooltip>
  <span v-else-if="placeholder" class="font-weight-medium">Não medida</span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { formatLatency, formatPercent } from '@/utils/formatters'
import { GRADE_COLOR, pingUnavailableReason, type WifiStationRow } from '@/utils/wifiPresentation'

const props = defineProps<{
  row: WifiStationRow
  /** Diz "Não medida" em vez de não mostrar nada — no card, onde há rótulo. */
  placeholder?: boolean
}>()

const store = useWifiClientsStore()
const state = computed(() => store.pingOf(props.row.deviceId, props.row.mac))
const unavailable = computed(() => pingUnavailableReason(props.row))

const resultText = computed(() => {
  const result = state.value?.result
  if (!result) return ''
  if (result.avgMs === null) return 'Sem resposta'
  const loss = result.lossPct > 0 ? ` · ${formatPercent(result.lossPct)} perdido` : ''
  return `${formatLatency(result.avgMs)}${loss}`
})
</script>
