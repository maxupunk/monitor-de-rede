<template>
  <v-card elevation="2" class="rounded-lg pa-6 mt-6">
    <div class="d-flex align-center justify-space-between mb-4">
      <div>
        <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
          <v-icon color="warning">mdi-bell-alert</v-icon>
          Histórico de Alertas
        </h2>
        <div class="text-subtitle-2 text-grey">
          Alertas disparados e normalizações deste monitor
        </div>
      </div>
      <div class="d-flex align-center ga-2">
        <v-btn
          size="small"
          variant="text"
          prepend-icon="mdi-refresh"
          :loading="loading"
          @click="emit('refresh')"
        >
          Atualizar
        </v-btn>
        <v-btn icon size="small" variant="text" @click="emit('toggle')">
          <v-icon>{{ show ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
          <v-tooltip activator="parent" location="top">
            {{ show ? 'Ocultar Alertas' : 'Mostrar Alertas' }}
          </v-tooltip>
        </v-btn>
      </div>
    </div>

    <v-expand-transition>
      <div v-if="show">
        <div
          class="history-scroll-container rounded-lg border overflow-y-auto"
          style="max-height: 450px"
        >
          <v-infinite-scroll
            :key="alertHistory.scrollKey.value"
            :height="420"
            @load="alertHistory.load"
          >
            <div class="table-responsive">
              <v-table density="comfortable" hover>
                <thead>
                  <tr>
                    <th style="width: 120px">Severidade</th>
                    <th style="width: 120px">Status</th>
                    <th>Título / Regra</th>
                    <th>Mensagem</th>
                    <th style="width: 180px">Início</th>
                    <th style="width: 180px">Normalizado em</th>
                    <th style="width: 160px">Ações</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in alertHistory.items.value" :key="item.id">
                    <td>
                      <v-chip :color="severityColor(item.severity)" size="x-small" variant="flat">
                        {{ severityLabel(item.severity).toUpperCase() }}
                      </v-chip>
                    </td>
                    <td>
                      <v-chip
                        :color="
                          item.status === 'resolved' ? 'success' : severityColor(item.severity)
                        "
                        size="x-small"
                        variant="tonal"
                      >
                        {{ statusLabel(item.status).toUpperCase() }}
                      </v-chip>
                    </td>
                    <td class="font-weight-medium">
                      {{ item.title || '—' }}
                    </td>
                    <td>
                      <span
                        :class="
                          item.status === 'active' ? 'text-error font-weight-medium' : 'text-body-2'
                        "
                      >
                        {{ item.message || '—' }}
                      </span>
                    </td>
                    <td>
                      {{ formatDateTime(item.startedAt, '—') }}
                    </td>
                    <td>
                      <span v-if="item.resolvedAt" class="text-success font-weight-medium">
                        {{ formatDateTime(item.resolvedAt, '—') }}
                      </span>
                      <span v-else class="text-grey">—</span>
                    </td>
                    <td>
                      <div v-if="item.status === 'active'" class="d-flex ga-1">
                        <v-btn
                          size="x-small"
                          variant="text"
                          prepend-icon="mdi-check-circle"
                          color="success"
                          :loading="alertsStoreLoading"
                          @click="emit('acknowledge', item)"
                        >
                          Reconhecer
                        </v-btn>
                        <v-menu location="bottom end">
                          <template #activator="{ props: menuProps }">
                            <v-btn
                              size="x-small"
                              variant="text"
                              prepend-icon="mdi-bell-off"
                              color="warning"
                              v-bind="menuProps"
                            >
                              Silenciar
                            </v-btn>
                          </template>
                          <v-list density="compact">
                            <v-list-item
                              v-for="duration in silenceDurations"
                              :key="duration.minutes"
                              :title="duration.label"
                              :disabled="alertsStoreLoading"
                              @click="emit('silence', item, duration.minutes)"
                            ></v-list-item>
                          </v-list>
                        </v-menu>
                      </div>
                      <v-chip
                        v-else-if="item.status === 'acknowledged'"
                        size="x-small"
                        color="info"
                        variant="tonal"
                        prepend-icon="mdi-check-circle"
                      >
                        Reconhecido
                      </v-chip>
                      <v-chip
                        v-else-if="item.status === 'silenced'"
                        size="x-small"
                        color="warning"
                        variant="tonal"
                        prepend-icon="mdi-bell-off"
                      >
                        Silenciado
                      </v-chip>
                      <span v-else class="text-grey">—</span>
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>
            <template #empty>
              <div class="text-caption text-grey text-center py-3">
                Nenhum outro registro no histórico de alertas.
              </div>
            </template>
          </v-infinite-scroll>
        </div>
      </div>
    </v-expand-transition>
  </v-card>
</template>

<script setup lang="ts">
import type { AlertEvent } from '@/stores/alerts'
import { severityColor, severityLabel, statusLabel } from '@/utils/alertPresentation'
import { formatDateTime } from '@/utils/formatters'
import type { useInfiniteList } from '@/composables/useInfiniteList'

defineProps<{
  show: boolean
  alertHistory: ReturnType<typeof useInfiniteList<AlertEvent>>
  loading: boolean
  alertsStoreLoading: boolean
  silenceDurations: Array<{ minutes: number; label: string }>
}>()

const emit = defineEmits<{
  (e: 'toggle'): void
  (e: 'refresh'): void
  (e: 'acknowledge', item: AlertEvent): void
  (e: 'silence', item: AlertEvent, minutes: number): void
}>()
</script>
