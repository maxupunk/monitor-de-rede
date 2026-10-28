<template>
  <div class="d-flex flex-column align-start ga-1">
    <router-link
      v-if="showDevice"
      :to="wifiTabOf(row)"
      class="d-inline-flex align-center ga-1 font-weight-bold text-primary"
    >
      <v-icon size="16">mdi-router-wireless</v-icon>
      {{ row.deviceName }}
    </router-link>
    <div class="d-flex flex-wrap align-center ga-1">
      <span class="font-weight-medium">{{ row.ssid ?? 'Rede sem nome' }}</span>
      <v-chip v-if="row.band" size="x-small" variant="flat" :color="BAND_COLOR[row.band]">
        {{ BAND_LABEL[row.band] }}
      </v-chip>
    </div>
    <span v-if="channel" class="text-body-2">{{ channel }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  BAND_COLOR,
  BAND_LABEL,
  channelText,
  type WifiStationRow,
  wifiTabOf,
} from '@/utils/wifiPresentation'

const props = defineProps<{
  row: WifiStationRow
  /** O roteador onde está — no painel geral, não na aba do próprio roteador. */
  showDevice?: boolean
}>()

const channel = computed(() => channelText(props.row))
</script>
