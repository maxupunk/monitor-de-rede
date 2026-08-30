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
      <div class="d-flex flex-column ga-2">
        <div class="d-flex align-center justify-space-between ga-2 flex-wrap mb-0.5">
          <div class="d-flex align-center ga-1.5 flex-wrap">
            <v-chip
              :color="severityColor(item.severity)"
              size="x-small"
              variant="flat"
              class="font-weight-bold text-uppercase px-2"
            >
              {{ severityLabel(item.severity) }}
            </v-chip>
            <v-chip color="success" variant="tonal" size="x-small" class="font-weight-medium">
              <v-icon start size="12">mdi-check-circle</v-icon>
              Resolvido
            </v-chip>
          </div>

          <span class="text-caption text-grey d-flex align-center ga-1 flex-shrink-0">
            <v-icon size="12">mdi-clock-check-outline</v-icon>
            {{ formatDateTime(item.resolvedAt || item.createdAt) }}
          </span>
        </div>

        <div class="text-subtitle-1 font-weight-bold text-break text-high-emphasis leading-tight">
          {{ item.title }}
        </div>
        <div class="text-body-2 text-grey-darken-1 text-break">
          {{ item.message }}
        </div>

        <div
          class="text-caption text-grey pt-2 mt-1 border-t d-flex align-center justify-space-between flex-wrap ga-1"
        >
          <span>Início: {{ formatDateTime(item.startedAt || item.createdAt) }}</span>
          <span v-if="item.resolvedAt" class="text-success font-weight-medium">
            Resolvido em {{ formatDateTime(item.resolvedAt) }}
          </span>
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
