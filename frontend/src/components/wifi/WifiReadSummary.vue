<template>
  <span class="d-inline-flex align-center ga-2">
    <template v-if="!snapshot">Sem leitura ainda.</template>
    <template v-else-if="snapshot.status === 'reading'">
      <v-progress-circular indeterminate size="14" width="2" color="primary" />
      Lendo o roteador agora…
    </template>
    <template v-else-if="snapshot.status === 'failed'">
      <v-icon size="16" color="error">mdi-alert-circle</v-icon>
      Falhou {{ formatRelativeTime(snapshot.readAt) }}
    </template>
    <template v-else>
      {{ stationsText }} · lido {{ formatRelativeTime(snapshot.readAt) }}
    </template>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import { formatRelativeTime } from '@/utils/formatters'

const props = defineProps<{ snapshot: WifiDeviceSnapshot | null }>()

const stationsText = computed(() => {
  const total = props.snapshot?.stations.length ?? 0
  if (total === 0) return 'Nenhum cliente'
  return total === 1 ? '1 cliente' : `${total} clientes`
})
</script>
