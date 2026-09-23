<template>
  <div>
    <PageHeader
      title="Histórico do host"
      subtitle="Médias de 1 minuto guardadas por 30 dias: CPU, memória, rede e disco do servidor e dos containers"
    >
      <template #actions>
        <v-btn-toggle
          v-model="range"
          mandatory
          color="primary"
          variant="outlined"
          density="comfortable"
          divided
        >
          <v-btn v-for="option in ranges" :key="option" :value="option">{{ option }}</v-btn>
        </v-btn-toggle>
      </template>
    </PageHeader>

    <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>

    <v-skeleton-loader v-if="loading && !history" type="card, card"></v-skeleton-loader>
    <template v-else-if="history">
      <v-alert
        v-if="history.host.length === 0 && history.containers.length === 0"
        type="info"
        variant="tonal"
        class="mb-4"
      >
        Ainda não há minutos registrados nesta janela. O primeiro aparece cerca de um minuto depois
        de o servidor começar a enviar métricas.
      </v-alert>

      <v-row v-if="history.host.length > 0" dense class="mb-2">
        <v-col cols="12" lg="6">
          <v-card rounded="xl" variant="outlined" class="h-100">
            <v-card-title class="text-subtitle-1">CPU do host</v-card-title>
            <v-card-text>
              <BaseMetricChart :series="cpuSeries" unit-type="percentage"></BaseMetricChart>
            </v-card-text>
          </v-card>
        </v-col>
        <v-col cols="12" lg="6">
          <v-card rounded="xl" variant="outlined" class="h-100">
            <v-card-title class="text-subtitle-1">Memória usada</v-card-title>
            <v-card-subtitle>{{ memoryCaption }}</v-card-subtitle>
            <v-card-text>
              <BaseMetricChart :series="memorySeries" unit-type="bytes"></BaseMetricChart>
            </v-card-text>
          </v-card>
        </v-col>
        <v-col cols="12" lg="6">
          <v-card rounded="xl" variant="outlined" class="h-100">
            <v-card-title class="text-subtitle-1">Tráfego de rede</v-card-title>
            <v-card-text>
              <BaseMetricChart :series="networkSeries" unit-type="bandwidth"></BaseMetricChart>
            </v-card-text>
          </v-card>
        </v-col>
        <v-col cols="12" lg="6">
          <v-card rounded="xl" variant="outlined" class="h-100">
            <v-card-title class="text-subtitle-1">Discos</v-card-title>
            <v-card-text>
              <div v-for="disk in latestDisks" :key="disk.mount" class="mb-3">
                <div class="d-flex justify-space-between text-body-2 mb-1">
                  <span class="text-truncate">{{ disk.mount }}</span>
                  <span>
                    {{ formatBinaryBytes(disk.usedBytes) }} /
                    {{ formatBinaryBytes(disk.totalBytes) }}
                  </span>
                </div>
                <v-progress-linear
                  :model-value="diskPercent(disk.usedBytes, disk.totalBytes)"
                  :color="diskPercent(disk.usedBytes, disk.totalBytes) > 90 ? 'error' : 'primary'"
                  height="8"
                  rounded
                ></v-progress-linear>
              </div>
              <div v-if="latestDisks.length === 0" class="text-medium-emphasis text-body-2">
                Nenhum disco informado por este host.
              </div>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>

      <v-card v-if="history.containers.length > 0" rounded="xl" variant="outlined">
        <v-card-title class="text-subtitle-1">Containers na janela</v-card-title>
        <v-table density="comfortable" hover>
          <thead>
            <tr>
              <th>Container</th>
              <th class="text-right">CPU média</th>
              <th class="text-right">CPU pico</th>
              <th class="text-right">Memória pico</th>
              <th class="text-right hidden-sm-and-down">Rede (rx / tx)</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="container in history.containers"
              :key="container.name"
              class="cursor-pointer"
              :class="{ 'selected-row': container.name === selectedContainer }"
              @click="openContainer(container.name)"
            >
              <td>
                <div class="font-weight-medium">{{ container.name }}</div>
                <div v-if="container.project" class="text-caption text-medium-emphasis">
                  {{ container.project }}
                </div>
              </td>
              <td class="text-right">{{ formatPercent(container.cpuAvg) }}</td>
              <td class="text-right">{{ formatPercent(container.cpuMax) }}</td>
              <td class="text-right">{{ formatBinaryBytes(container.memoryMax) }}</td>
              <td class="text-right hidden-sm-and-down">
                {{ formatDecimalBytes(container.netRxBytes) }} /
                {{ formatDecimalBytes(container.netTxBytes) }}
              </td>
            </tr>
          </tbody>
        </v-table>
      </v-card>

      <v-card v-if="containerHistory" rounded="xl" variant="outlined" class="mt-4">
        <v-card-title class="d-flex align-center">
          <v-icon class="mr-2" color="primary">mdi-cube-outline</v-icon>
          {{ containerHistory.containerName }}
          <v-spacer></v-spacer>
          <v-btn icon="mdi-close" variant="text" size="small" @click="closeContainer"></v-btn>
        </v-card-title>
        <v-card-text>
          <v-row>
            <v-col cols="12" lg="6">
              <div class="text-subtitle-2 mb-2">CPU</div>
              <BaseMetricChart
                :series="containerCpuSeries"
                unit-type="percentage"
              ></BaseMetricChart>
            </v-col>
            <v-col cols="12" lg="6">
              <div class="text-subtitle-2 mb-2">Memória</div>
              <BaseMetricChart :series="containerMemorySeries" unit-type="bytes"></BaseMetricChart>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import PageHeader from '@/components/PageHeader.vue'
import { useDockerStore } from '@/stores/docker'
import { formatBinaryBytes, formatDecimalBytes, formatPercent } from '@/utils/formatters'
import type { ContainerHistoryResponse } from '@/bindings/ContainerHistoryResponse'
import type { HostHistoryResponse } from '@/bindings/HostHistoryResponse'
import type { MetricsRange } from '@/bindings/MetricsRange'

const docker = useDockerStore()

const ranges: MetricsRange[] = ['1h', '6h', '24h', '7d', '30d']
const range = ref<MetricsRange>('24h')
const history = ref<HostHistoryResponse | null>(null)
const containerHistory = ref<ContainerHistoryResponse | null>(null)
const selectedContainer = ref<string | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

/** Consulta sob demanda (histórico não-live): muda só com host ou janela. */
async function load() {
  loading.value = true
  error.value = null
  try {
    history.value = await docker.api.history(range.value)
    if (selectedContainer.value) await openContainer(selectedContainer.value)
  } catch (reason: unknown) {
    error.value = reason instanceof Error ? reason.message : 'Erro ao carregar o histórico'
  } finally {
    loading.value = false
  }
}

async function openContainer(name: string) {
  selectedContainer.value = name
  try {
    containerHistory.value = await docker.api.containerHistory(name, range.value)
  } catch (reason: unknown) {
    error.value = reason instanceof Error ? reason.message : 'Erro ao carregar o container'
  }
}

function closeContainer() {
  selectedContainer.value = null
  containerHistory.value = null
}

function diskPercent(used: number, total: number): number {
  return total > 0 ? (used / total) * 100 : 0
}

const latestDisks = computed(() => history.value?.host.at(-1)?.disks ?? [])

const memoryCaption = computed(() => {
  const total = history.value?.host.at(-1)?.memoryTotal
  return total ? `de ${formatBinaryBytes(total)}` : ''
})

const cpuSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'host-cpu-avg',
    label: 'Média',
    color: '#2196F3',
    fillArea: true,
    data: (history.value?.host ?? []).map((point) => ({ time: point.at, value: point.cpuAvg })),
  },
  {
    id: 'host-cpu-max',
    label: 'Pico',
    color: '#FF7043',
    data: (history.value?.host ?? []).map((point) => ({ time: point.at, value: point.cpuMax })),
  },
])

const memorySeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'host-memory',
    label: 'Usada',
    color: '#7E57C2',
    fillArea: true,
    data: (history.value?.host ?? []).map((point) => ({
      time: point.at,
      value: point.memoryUsed,
      formattedValue: formatBinaryBytes(point.memoryUsed),
    })),
  },
])

const networkSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'host-rx',
    label: 'Recebido',
    color: '#26A69A',
    data: (history.value?.host ?? []).map((point) => ({
      time: point.at,
      value: point.netRxBps * 8,
    })),
  },
  {
    id: 'host-tx',
    label: 'Enviado',
    color: '#FFA726',
    data: (history.value?.host ?? []).map((point) => ({
      time: point.at,
      value: point.netTxBps * 8,
    })),
  },
])

const containerCpuSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'container-cpu-avg',
    label: 'Média',
    color: '#2196F3',
    fillArea: true,
    data: (containerHistory.value?.points ?? []).map((point) => ({
      time: point.at,
      value: point.cpuAvg,
    })),
  },
  {
    id: 'container-cpu-max',
    label: 'Pico',
    color: '#FF7043',
    data: (containerHistory.value?.points ?? []).map((point) => ({
      time: point.at,
      value: point.cpuMax,
    })),
  },
])

const containerMemorySeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'container-memory',
    label: 'Pico',
    color: '#7E57C2',
    fillArea: true,
    data: (containerHistory.value?.points ?? []).map((point) => ({
      time: point.at,
      value: point.memoryMax,
      formattedValue: formatBinaryBytes(point.memoryMax),
    })),
  },
])

watch(
  () => [docker.selectedHostKey, range.value],
  () => {
    closeContainer()
    load()
  },
  { immediate: true }
)
</script>

<style scoped>
.cursor-pointer {
  cursor: pointer;
}

.selected-row {
  background: rgba(var(--v-theme-primary), 0.08);
}
</style>
