<template>
  <ResponsiveDataTable
    :headers="resolvedHeaders"
    :items="alertsStore.resolvedAlerts"
    :loading="alertsStore.loading"
    :items-per-page="-1"
    hide-default-footer
    no-data-text="Nenhum alerta resolvido na sessão atual."
    :clickable="false"
  >
    <template #item.severity="{ item }">
      <v-chip :color="severityColor(item.severity)" size="small">
        {{ severityLabel(item.severity) }}
      </v-chip>
    </template>

    <template #item.createdAt="{ item }">
      {{ formatDateTime(item.startedAt || item.createdAt) }}
    </template>

    <template #item.resolvedAt="{ item }">
      <v-chip color="success" variant="tonal" size="small">
        <v-icon start size="14">mdi-check-circle</v-icon>
        {{ item.resolvedAt ? formatDateTime(item.resolvedAt) : 'Resolvido' }}
      </v-chip>
    </template>

    <template #mobile-item="{ item }">
      <div class="d-flex flex-column ga-2 pa-1">
        <div class="d-flex align-center ga-2">
          <v-chip :color="severityColor(item.severity)" size="x-small">
            {{ severityLabel(item.severity) }}
          </v-chip>
          <v-chip color="success" variant="tonal" size="x-small"> Resolvido </v-chip>
        </div>
        <div class="text-subtitle-1 font-weight-bold">{{ item.title }}</div>
        <div class="text-body-2 text-grey-darken-1">{{ item.message }}</div>
        <div class="text-caption text-grey">
          Início: {{ formatDateTime(item.startedAt || item.createdAt) }} | Resolvido:
          {{ item.resolvedAt ? formatDateTime(item.resolvedAt) : 'Sim' }}
        </div>
      </div>
    </template>
  </ResponsiveDataTable>
</template>

<script setup lang="ts">
import { useAlertsStore } from '@/stores/alerts'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { severityLabel, severityColor } from '@/utils/alertPresentation'
import { formatDateTime } from '@/utils/formatters'

const alertsStore = useAlertsStore()

const resolvedHeaders = [
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Título', key: 'title' },
  { title: 'Mensagem', key: 'message' },
  { title: 'Data de Início', key: 'createdAt', width: '170px' },
  { title: 'Resolvido Em', key: 'resolvedAt', width: '170px' },
]
</script>
