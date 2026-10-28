<template>
  <div class="d-flex flex-column align-start ga-1">
    <v-chip
      size="small"
      variant="flat"
      :color="GRADE_COLOR[row.signalGrade]"
      :prepend-icon="signalIcon(row.signalGrade)"
      class="font-weight-bold"
    >
      {{ row.signalDbm }} dBm
    </v-chip>
    <span v-if="row.snrDb !== null && row.snrGrade" class="text-body-2">
      SNR
      <span :class="`text-${GRADE_COLOR[row.snrGrade]} font-weight-bold`">{{ row.snrDb }} dB</span>
    </span>
  </div>
</template>

<script setup lang="ts">
import { GRADE_COLOR, signalIcon, type WifiStationRow } from '@/utils/wifiPresentation'

defineProps<{ row: WifiStationRow }>()
</script>
