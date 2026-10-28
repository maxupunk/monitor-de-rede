<template>
  <v-chip
    size="small"
    variant="flat"
    :color="HEALTH[health].color"
    :prepend-icon="HEALTH[health].icon"
    class="font-weight-bold"
  >
    {{ HEALTH[health].label }}
  </v-chip>
</template>

<script setup lang="ts">
import type { WifiHealth } from '@/bindings/WifiHealth'
import { HEALTH } from '@/utils/wifiPresentation'

defineProps<{ health: WifiHealth }>()
</script>
