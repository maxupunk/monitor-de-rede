<template>
  <div class="d-flex align-center justify-space-between mb-3 flex-wrap ga-2">
    <div class="text-body-2 text-grey-darken-1">
      Todos os alertas já registrados, do mais recente para o mais antigo.
    </div>
    <v-chip v-if="history.total.value > 0" size="small" variant="outlined" color="primary">
      {{ history.items.value.length }} de {{ history.total.value }}
    </v-chip>
  </div>

  <v-infinite-scroll :key="history.scrollKey.value" @load="history.load">
    <!-- Desktop: Tabela -->
    <div v-if="$vuetify.display.mdAndUp" class="table-responsive">
      <v-table hover density="comfortable" class="rounded-lg border">
        <thead>
          <tr>
            <th style="width: 110px">Severidade</th>
            <th style="width: 120px">Situação</th>
            <th>Alerta</th>
            <th>Mensagem</th>
            <th style="width: 170px">Início</th>
            <th style="width: 170px">Normalizado em</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="alert in history.items.value" :key="alert.id">
            <td>
              <v-chip
                :color="severityColor(alert.severity)"
                size="x-small"
                variant="flat"
                class="font-weight-bold text-uppercase px-2"
              >
                {{ severityLabel(alert.severity) }}
              </v-chip>
            </td>
            <td>
              <v-chip :color="statusColor(alert.status)" variant="tonal" size="x-small">
                {{ statusLabel(alert.status) }}
              </v-chip>
            </td>
            <td class="font-weight-medium">
              {{ alert.title }}
              <v-chip
                v-if="problemKindLabel(alert.data?.problemKind)"
                size="x-small"
                variant="tonal"
                color="grey"
                class="ml-2"
              >
                {{ problemKindLabel(alert.data?.problemKind) }}
              </v-chip>
            </td>
            <td class="text-body-2">{{ alert.message || '—' }}</td>
            <td>{{ formatDateTime(alert.startedAt || alert.createdAt) }}</td>
            <td>
              <span v-if="alert.resolvedAt">{{ formatDateTime(alert.resolvedAt) }}</span>
              <span v-else class="text-grey">Em aberto</span>
            </td>
          </tr>
        </tbody>
      </v-table>
    </div>

    <!-- Mobile: Cards Responsivos -->
    <div v-else class="d-flex flex-column ga-2">
      <v-card v-for="alert in history.items.value" :key="alert.id" border rounded="lg" class="pa-3">
        <div class="d-flex align-center justify-space-between ga-2 flex-wrap mb-1">
          <div class="d-flex align-center ga-1.5 flex-wrap">
            <v-chip
              :color="severityColor(alert.severity)"
              size="x-small"
              variant="flat"
              class="font-weight-bold text-uppercase px-2"
            >
              {{ severityLabel(alert.severity) }}
            </v-chip>
            <v-chip :color="statusColor(alert.status)" variant="tonal" size="x-small">
              {{ statusLabel(alert.status) }}
            </v-chip>
            <v-chip
              v-if="problemKindLabel(alert.data?.problemKind)"
              size="x-small"
              variant="tonal"
              color="grey"
            >
              {{ problemKindLabel(alert.data?.problemKind) }}
            </v-chip>
          </div>
          <span class="text-caption text-grey d-flex align-center ga-1">
            <v-icon size="12">mdi-clock-outline</v-icon>
            {{ formatDateTime(alert.startedAt || alert.createdAt) }}
          </span>
        </div>

        <div class="text-subtitle-1 font-weight-bold text-break text-high-emphasis leading-tight">
          {{ alert.title }}
        </div>
        <div v-if="alert.message" class="text-body-2 text-grey-darken-1 text-break mt-1">
          {{ alert.message }}
        </div>

        <div
          class="text-caption text-grey pt-2 mt-1 border-t d-flex align-center justify-space-between flex-wrap ga-1"
        >
          <span>Início: {{ formatDateTime(alert.startedAt || alert.createdAt) }}</span>
          <span>
            Normalizado:
            <strong v-if="alert.resolvedAt" class="text-success font-weight-medium">
              {{ formatDateTime(alert.resolvedAt) }}
            </strong>
            <span v-else class="text-warning">Em aberto</span>
          </span>
        </div>
      </v-card>
    </div>

    <template #empty>
      <div class="text-caption text-grey text-center py-4">Nenhum outro alerta no histórico.</div>
    </template>
  </v-infinite-scroll>
</template>

<script setup lang="ts">
import { useInfiniteList } from '@/composables/useInfiniteList'
import type { AlertEvent } from '@/stores/alerts'
import {
  severityLabel,
  severityColor,
  statusLabel,
  statusColor,
  problemKindLabel,
} from '@/utils/alertPresentation'
import { formatDateTime } from '@/utils/formatters'

const history = useInfiniteList<AlertEvent>(() => '/alerts', { label: 'histórico de alertas' })

defineExpose({ history })
</script>
