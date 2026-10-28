<template>
  <div class="text-no-wrap">
    <div :aria-label="`Envio ${rate(row.txRateBps)}`">
      <v-icon size="14" color="primary">mdi-arrow-up</v-icon> {{ rate(row.txRateBps) }}
    </div>
    <div :aria-label="`Recebimento ${rate(row.rxRateBps)}`">
      <v-icon size="14" color="secondary">mdi-arrow-down</v-icon> {{ rate(row.rxRateBps) }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { formatBps } from '@/utils/formatters'
import type { WifiStationRow } from '@/utils/wifiPresentation'

defineProps<{ row: WifiStationRow }>()

function rate(bps: number | null): string {
  return bps === null ? '—' : formatBps(bps)
}
</script>
