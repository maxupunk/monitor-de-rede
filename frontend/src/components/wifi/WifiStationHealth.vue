<template>
  <div class="d-flex flex-column align-start ga-1">
    <WifiHealthChip :health="row.health" />
    <span v-if="row.healthReason" class="text-body-2">{{ row.healthReason }}</span>
    <span v-if="details" class="text-caption">{{ details }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { stationDetails, type WifiStationRow } from '@/utils/wifiPresentation'
import WifiHealthChip from './WifiHealthChip.vue'

const props = defineProps<{ row: WifiStationRow }>()

const details = computed(() => stationDetails(props.row))
</script>
