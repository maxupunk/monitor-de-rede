<template>
  <div
    class="d-flex flex-column flex-sm-row align-start align-sm-center justify-space-between ga-3 mb-4"
  >
    <v-btn-toggle
      v-model="subFilter"
      density="compact"
      variant="outlined"
      color="primary"
      mandatory
    >
      <v-btn value="all" size="small"> Todos ({{ alertsStore.activeAlerts.length }}) </v-btn>
      <v-btn value="unacknowledged" size="small">
        Não Reconhecidos ({{ alertsStore.pendingAlerts.length }})
      </v-btn>
      <v-btn value="acknowledged" size="small">
        Reconhecidos ({{ alertsStore.acknowledgedAlerts.length }})
      </v-btn>
    </v-btn-toggle>

    <v-btn
      v-if="alertsStore.activeAlerts.length > 0"
      size="small"
      color="primary"
      variant="tonal"
      prepend-icon="mdi-refresh"
      :loading="verifyingAll"
      @click="emit('verify-all')"
    >
      Verificar Todos os Pendentes
    </v-btn>
  </div>

  <ResponsiveDataTable
    :headers="activeHeaders"
    :items="filteredAlerts"
    :loading="alertsStore.loading"
    :items-per-page="-1"
    hide-default-footer
    no-data-text="Nenhum alerta pendente no momento!"
    :clickable="false"
  >
    <template #item.severity="{ item }">
      <v-chip :color="severityColor(item.severity)" size="small">
        {{ severityLabel(item.severity) }}
      </v-chip>
    </template>

    <template #item.status="{ item }">
      <v-chip :color="statusColor(item.status)" variant="outlined" size="small">
        {{ statusLabel(item.status) }}
      </v-chip>
    </template>

    <template #item.message="{ item }">
      <div>
        <v-chip
          v-if="problemKindLabel(item.data?.problemKind)"
          size="x-small"
          variant="tonal"
          color="grey"
          class="mr-2"
        >
          {{ problemKindLabel(item.data?.problemKind) }}
        </v-chip>
        {{ item.message }}
      </div>
      <div v-if="episodeInfo(item)" class="text-caption text-warning">
        {{ episodeInfo(item) }}
      </div>
    </template>

    <template #item.createdAt="{ item }">
      {{ formatDateTime(item.startedAt || item.createdAt) }}
    </template>

    <template #item.actions="{ item }">
      <div class="d-flex align-center ga-1 flex-wrap">
        <v-btn
          size="small"
          color="primary"
          variant="tonal"
          :disabled="item.status === 'acknowledged'"
          :loading="verifyingId === item.id"
          @click="emit('acknowledge', item.id)"
        >
          Reconhecer
        </v-btn>
        <v-btn
          size="small"
          color="info"
          variant="outlined"
          :loading="verifyingId === item.id"
          @click="emit('verify', item.id)"
        >
          Verificar
        </v-btn>
        <v-btn
          size="small"
          color="warning"
          variant="outlined"
          :disabled="item.status === 'silenced'"
          @click="emit('silence', item.id)"
        >
          Silenciar
        </v-btn>
        <v-btn
          size="small"
          color="secondary"
          variant="outlined"
          prepend-icon="mdi-source-branch"
          @click="emit('correlate', item.id)"
        >
          Correlação
        </v-btn>
      </div>
    </template>

    <template #mobile-item="{ item }">
      <div class="d-flex flex-column ga-2">
        <!-- Top Row: Badges de Severidade, Status e Hora -->
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
            <v-chip
              :color="statusColor(item.status)"
              variant="tonal"
              size="x-small"
              class="font-weight-medium"
            >
              {{ statusLabel(item.status) }}
            </v-chip>
            <v-chip
              v-if="problemKindLabel(item.data?.problemKind)"
              size="x-small"
              variant="tonal"
              color="grey"
              class="font-weight-medium"
            >
              {{ problemKindLabel(item.data?.problemKind) }}
            </v-chip>
          </div>

          <span class="text-caption text-grey d-flex align-center ga-1 flex-shrink-0">
            <v-icon size="12">mdi-clock-outline</v-icon>
            {{ formatDateTime(item.startedAt || item.createdAt) }}
          </span>
        </div>

        <!-- Middle: Título e Mensagem do Alerta -->
        <div class="d-flex flex-column ga-1">
          <div class="text-subtitle-1 font-weight-bold text-break text-high-emphasis leading-tight">
            {{ item.title }}
          </div>
          <div class="text-body-2 text-grey-darken-1 text-break">
            {{ item.message }}
          </div>
          <v-alert
            v-if="episodeInfo(item)"
            type="warning"
            variant="tonal"
            density="compact"
            class="py-1 px-2 my-1 text-caption rounded"
          >
            {{ episodeInfo(item) }}
          </v-alert>
        </div>

        <!-- Footer: Barra de Ações com Padrão Visual Harmonioso -->
        <div class="d-flex align-center flex-wrap ga-1.5 pt-2 mt-1 border-t">
          <v-btn
            size="small"
            color="primary"
            variant="flat"
            prepend-icon="mdi-check"
            :disabled="item.status === 'acknowledged'"
            :loading="verifyingId === item.id"
            class="text-caption px-2"
            @click="emit('acknowledge', item.id)"
          >
            Reconhecer
          </v-btn>
          <v-btn
            size="small"
            color="info"
            variant="tonal"
            prepend-icon="mdi-refresh"
            :loading="verifyingId === item.id"
            class="text-caption px-2"
            @click="emit('verify', item.id)"
          >
            Verificar
          </v-btn>
          <v-btn
            size="small"
            color="warning"
            variant="tonal"
            prepend-icon="mdi-volume-off"
            :disabled="item.status === 'silenced'"
            class="text-caption px-2"
            @click="emit('silence', item.id)"
          >
            Silenciar
          </v-btn>
          <v-btn
            size="small"
            color="secondary"
            variant="tonal"
            prepend-icon="mdi-source-branch"
            class="text-caption px-2"
            @click="emit('correlate', item.id)"
          >
            Correlação
          </v-btn>
        </div>
      </div>
    </template>
  </ResponsiveDataTable>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAlertsStore, type AlertEvent } from '@/stores/alerts'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import {
  severityLabel,
  severityColor,
  statusLabel,
  statusColor,
  problemKindLabel,
} from '@/utils/alertPresentation'
import { formatDateTime, formatRelativeTime } from '@/utils/formatters'

defineProps<{
  verifyingId: number | null
  verifyingAll: boolean
}>()

const emit = defineEmits<{
  acknowledge: [id: number]
  verify: [id: number]
  silence: [id: number]
  'verify-all': []
  correlate: [id: number]
}>()

const alertsStore = useAlertsStore()

const subFilter = defineModel<'all' | 'unacknowledged' | 'acknowledged'>('subFilter', {
  default: 'all',
})

const filteredAlerts = computed(() => {
  if (subFilter.value === 'unacknowledged') {
    return alertsStore.pendingAlerts
  }
  if (subFilter.value === 'acknowledged') {
    return alertsStore.acknowledgedAlerts
  }
  return alertsStore.activeAlerts
})

const activeHeaders = [
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Título', key: 'title' },
  { title: 'Mensagem', key: 'message' },
  { title: 'Status', key: 'status', width: '130px' },
  { title: 'Data/Hora', key: 'createdAt', width: '170px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '280px' },
]

function episodeInfo(alert: AlertEvent): string {
  if (alert.status !== 'recovering' && alert.status !== 'flapping') return ''
  const parts: string[] = []
  if (alert.status === 'flapping' && alert.data?.flappingSince) {
    parts.push(`oscilando desde ${formatRelativeTime(alert.data.flappingSince)}`)
  }
  if (alert.data?.lastProblemAt) {
    parts.push(`último problema ${formatRelativeTime(alert.data.lastProblemAt)}`)
  }
  const recurrences = alert.data?.recurrenceCount ?? 0
  if (recurrences > 0) {
    parts.push(`${recurrences} ${recurrences === 1 ? 'recaída' : 'recaídas'}`)
  }
  return parts.join(' · ')
}
</script>
