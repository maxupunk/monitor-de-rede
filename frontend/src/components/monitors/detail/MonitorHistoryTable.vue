<template>
  <v-card elevation="2" class="rounded-lg pa-6">
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
            <div class="table-responsive">
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
