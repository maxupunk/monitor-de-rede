<template>
  <div class="min-w-0">
    <div class="font-weight-bold text-break">
      <v-icon v-if="watched" size="16" color="primary" class="mr-1">mdi-eye</v-icon>
      {{ watched?.name ?? stationName(row) }}
    </div>
    <div class="text-body-2 d-flex flex-wrap ga-1">
      <span v-if="row.ip" class="font-weight-medium">{{ row.ip }}</span>
      <span class="text-mono">{{ row.mac }}</span>
    </div>
    <div v-if="row.hostname && row.vendor" class="text-caption">{{ row.vendor }}</div>
    <div v-else-if="row.randomizedMac && !row.vendor" class="text-caption">
      MAC privado: o aparelho esconde o fabricante
    </div>
    <v-chip
      v-if="!watched && looksFixed(row)"
      size="x-small"
      color="info"
      variant="tonal"
      prepend-icon="mdi-lightbulb-on-outline"
      class="mt-1"
    >
      Sempre conectado — vale vigiar
    </v-chip>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { looksFixed, stationName, type WifiStationRow } from '@/utils/wifiPresentation'

const props = defineProps<{ row: WifiStationRow }>()

const store = useWifiClientsStore()
const watched = computed(() => store.watchedByMac.get(props.row.mac) ?? null)
</script>

<style scoped>
.text-mono {
  font-family: monospace;
}
</style>
