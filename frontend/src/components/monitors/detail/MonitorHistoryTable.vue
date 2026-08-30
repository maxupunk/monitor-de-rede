<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-md-6">
    <div class="d-flex align-center justify-space-between mb-4">
      <div>
        <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
          <v-icon color="primary">mdi-history</v-icon>
          Histórico de Execuções Recentes
        </h2>
        <div class="text-subtitle-2 text-grey">Resultados das últimas verificações</div>
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
            {{ show ? 'Ocultar Histórico' : 'Mostrar Histórico' }}
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
          <v-infinite-scroll :key="history.scrollKey.value" :height="420" @load="history.load">
            <!-- Desktop: Tabela -->
            <div v-if="$vuetify.display.mdAndUp" class="table-responsive">
              <v-table density="comfortable" hover>
                <thead>
                  <tr>
                    <th style="width: 110px">Status</th>
                    <th style="width: 140px">Latência (Ping)</th>
                    <th style="width: 120px">Duração</th>
                    <th style="width: 180px">Data e Hora</th>
                    <th>Mensagem</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in history.items.value" :key="item.id">
                    <td>
                      <v-chip :color="getStatusColor(item.status)" size="x-small" variant="flat">
                        {{ item.status ? item.status.toUpperCase() : 'UNKNOWN' }}
                      </v-chip>
                    </td>
                    <td>
                      <span
                        v-if="item.latencyMs !== undefined && item.latencyMs !== null"
                        class="font-weight-medium"
                      >
                        {{ formatLatency(item.latencyMs) }}
                      </span>
                      <span v-else class="text-grey">-</span>
                    </td>
                    <td>
                      <span class="text-grey">{{ item.durationMs }} ms</span>
                    </td>
                    <td>
                      <span>{{ formatDateTime(item.finishedAt, '-') }}</span>
                    </td>
                    <td>
                      <span
                        :class="
                          item.status === 'down' ? 'text-error font-weight-medium' : 'text-body-2'
                        "
                      >
                        {{ item.message || '-' }}
                      </span>
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>

            <!-- Mobile: Cards Responsivos -->
            <div v-else class="d-flex flex-column ga-2 pa-2">
              <v-card
                v-for="item in history.items.value"
                :key="item.id"
                border
                rounded="lg"
                class="pa-3"
              >
                <div class="d-flex align-center justify-space-between ga-2 mb-1">
                  <v-chip
                    :color="getStatusColor(item.status)"
                    size="x-small"
                    variant="flat"
                    class="font-weight-bold text-uppercase px-2"
                  >
                    {{ item.status ? item.status.toUpperCase() : 'UNKNOWN' }}
                  </v-chip>
                  <span class="text-caption text-grey d-flex align-center ga-1">
                    <v-icon size="12">mdi-clock-outline</v-icon>
                    {{ formatDateTime(item.finishedAt, '-') }}
                  </span>
                </div>

                <div
                  class="d-flex flex-wrap align-center justify-space-between ga-2 text-caption text-grey my-1"
                >
                  <span>
                    Latência:
                    <strong
                      v-if="item.latencyMs !== undefined && item.latencyMs !== null"
                      class="text-high-emphasis font-weight-bold"
                    >
                      {{ formatLatency(item.latencyMs) }}
                    </strong>
                    <span v-else>—</span>
                  </span>
                  <span>Duração: {{ item.durationMs }} ms</span>
                </div>

                <div
                  v-if="item.message"
                  :class="
                    item.status === 'down'
                      ? 'text-error font-weight-medium'
                      : 'text-body-2 text-grey-darken-1'
                  "
                  class="pt-1 mt-1 border-t text-break"
                >
                  {{ item.message }}
                </div>
              </v-card>
            </div>

            <template #empty>
              <div class="text-caption text-grey text-center py-3">
                Nenhum outro registro no histórico.
              </div>
            </template>
          </v-infinite-scroll>
        </div>
      </div>
    </v-expand-transition>
  </v-card>
</template>

<script setup lang="ts">
import type { MonitorResult } from '@/stores/monitors'
import { getStatusColor } from '@/utils/monitorPresentation'
import { formatDateTime, formatLatency } from '@/utils/formatters'
import type { useInfiniteList } from '@/composables/useInfiniteList'

defineProps<{
  show: boolean
  history: ReturnType<typeof useInfiniteList<MonitorResult>>
  loading: boolean
}>()

const emit = defineEmits<{
  (e: 'toggle'): void
  (e: 'refresh'): void
}>()
</script>
