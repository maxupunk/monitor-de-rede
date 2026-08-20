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
      :monitors="detailStore.monitors"
      :loading="detailStore.loading"
      variant="device"
      no-data-text="Nenhum monitor configurado para este equipamento. Use &quot;Novo monitor&quot; ou &quot;Configurar Monitoramento&quot; para descobrir automaticamente."
      @edit="emit('openMonitorDialog', $event)"
      @changed="emit('reloadMonitors')"
    ></MonitorsTable>
  </div>
</template>

<script setup lang="ts">
import { useDeviceDetailStore, type DeviceMonitor } from '@/stores/deviceDetail'
import MonitorsTable from '@/components/MonitorsTable.vue'

const emit = defineEmits<{
  (e: 'openMonitorDialog', monitor?: DeviceMonitor): void
  (e: 'reloadMonitors'): void
}>()

const detailStore = useDeviceDetailStore()
</script>
