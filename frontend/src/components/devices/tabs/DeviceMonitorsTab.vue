<template>
  <div>
    <v-alert
      v-if="detailStore.capabilities?.reachMonitorBlockedReason"
      type="info"
      variant="tonal"
      density="comfortable"
      class="mb-4 rounded-lg"
    >
      {{ detailStore.capabilities.reachMonitorBlockedReason }}
    </v-alert>
    <MonitorsTable
      :monitors="orderedMonitors"
      :loading="detailStore.loading"
      variant="device"
      no-data-text='Nenhum monitor configurado para este equipamento. Use "Novo monitor" ou "Configurar Monitoramento" para descobrir automaticamente.'
      @edit="emit('openMonitorDialog', $event)"
      @changed="emit('reloadMonitors')"
    ></MonitorsTable>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDeviceDetailStore, type DeviceMonitor } from '@/stores/deviceDetail'
import MonitorsTable from '@/components/MonitorsTable.vue'

const emit = defineEmits<{
  (e: 'openMonitorDialog', monitor?: DeviceMonitor): void
  (e: 'reloadMonitors'): void
}>()

const detailStore = useDeviceDetailStore()

/**
 * A ordem pertence a esta aba, não à store compartilhada: a página global
 * de monitores continua livre para aplicar os próprios filtros e ordenação.
 * A cópia evita mutar a resposta do dispositivo e o sort estável preserva a
 * ordem original dentro dos grupos ativo e inativo.
 */
const orderedMonitors = computed(() =>
  [...detailStore.monitors].sort(
    (first, second) => Number(second.isEnabled !== false) - Number(first.isEnabled !== false)
  )
)
</script>
