<template>
  <div>
    <div class="d-flex flex-wrap align-center justify-space-between ga-2 mb-4">
      <div class="text-body-2">Varreduras anteriores ficam aqui para auditoria.</div>
      <v-btn
        size="small"
        color="error"
        variant="tonal"
        prepend-icon="mdi-delete-sweep"
        :loading="cleaning"
        @click="emit('cleanup')"
      >
        Limpar histórico antigo
      </v-btn>
    </div>

    <ResponsiveDataTable
      :headers="HEADERS"
      :items="runs"
      :loading="loading"
      :items-per-page="25"
      no-data-text="Nenhuma varredura registrada."
      :clickable="false"
    >
      <template #item.network="{ item }">
        <div class="font-weight-medium">{{ networkName(item) }}</div>
        <div class="text-caption font-mono">{{ item.cidr || '—' }}</div>
      </template>

      <template #item.status="{ item }">
        <v-chip :color="runStatus(item.status).color" size="small" variant="flat">
          {{ runStatus(item.status).label }}
        </v-chip>
      </template>

      <template #item.startedAt="{ item }">
        {{ formatDateTime(item.startedAt) }}
      </template>

      <template #item.duration="{ item }">
        {{ duration(item) }}
      </template>

      <template #mobile-item="{ item }">
        <div class="d-flex align-start justify-space-between ga-2">
          <div class="min-w-0">
            <div class="text-subtitle-1 font-weight-bold text-truncate">
              {{ networkName(item) }}
            </div>
            <div class="text-caption font-mono">{{ item.cidr || '—' }}</div>
            <div class="d-flex flex-wrap align-center ga-2 mt-1">
              <v-chip :color="runStatus(item.status).color" size="x-small" variant="flat">
                {{ runStatus(item.status).label }}
              </v-chip>
              <span class="text-caption">{{ formatDateTime(item.startedAt) }}</span>
            </div>
          </div>
          <div class="text-body-2 font-weight-bold text-no-wrap">
            {{ item.devicesFound }} encontrado(s)
          </div>
        </div>
      </template>
    </ResponsiveDataTable>
  </div>
</template>

<script setup lang="ts">
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import type { DiscoveryRun } from '@/stores/discovery'
import { formatDateTime, formatElapsedMs } from '@/utils/formatters'

defineProps<{
  runs: DiscoveryRun[]
  loading: boolean
  cleaning: boolean
}>()

const emit = defineEmits<{ cleanup: [] }>()

const HEADERS = [
  { title: 'Rede', key: 'network', sortable: false },
  { title: 'Encontrados', key: 'devicesFound', width: '120px' },
  { title: 'Status', key: 'status', width: '130px' },
  { title: 'Iniciada em', key: 'startedAt', width: '170px' },
  { title: 'Duração', key: 'duration', width: '110px', sortable: false },
]

const RUN_STATUS: Record<string, { label: string; color: string }> = {
  pending: { label: 'Na fila', color: 'info' },
  running: { label: 'Em execução', color: 'primary' },
  completed: { label: 'Concluída', color: 'success' },
  cancelled: { label: 'Cancelada', color: 'warning' },
  failed: { label: 'Falhou', color: 'error' },
}

function runStatus(status: string) {
  return RUN_STATUS[status] ?? { label: status, color: 'info' }
}

function networkName(run: DiscoveryRun): string {
  return run.networkName || `Rede #${run.networkId}`
}

function duration(run: DiscoveryRun): string {
  if (!run.startedAt || !run.finishedAt) return '—'
  return formatElapsedMs(new Date(run.finishedAt).getTime() - new Date(run.startedAt).getTime())
}
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
</style>
