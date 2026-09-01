<template>
  <div>
    <PageHeader
      title="Docker"
      subtitle="Saúde da Engine, inventário e consumo dos containers em tempo quase real"
    >
      <template #actions>
        <v-btn
          color="primary"
          prepend-icon="mdi-refresh"
          :loading="docker.loading"
          @click="docker.refreshAll()"
        >
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="docker.error" type="error" variant="tonal" closable class="mb-4">
      {{ docker.error }}
    </v-alert>
    <v-alert
      v-if="docker.metrics?.failedContainerCount"
      type="warning"
      variant="tonal"
      class="mb-4"
    >
      Não foi possível coletar a amostra de
      {{ docker.metrics.failedContainerCount }} container(s). Os totais abaixo são parciais.
    </v-alert>
    <template v-if="docker.available">
      <v-row dense class="mb-2">
        <v-col v-for="card in summaryCards" :key="card.label" cols="6" md="3">
          <v-card rounded="xl" variant="outlined" class="pa-4 h-100">
            <div class="d-flex align-center justify-space-between mb-2">
              <v-avatar :color="card.color" variant="tonal" size="38">
                <v-icon>{{ card.icon }}</v-icon>
              </v-avatar>
              <span class="text-h5 font-weight-bold">{{ card.value }}</span>
            </div>
            <div class="text-caption text-medium-emphasis">{{ card.label }}</div>
          </v-card>
        </v-col>
      </v-row>

      <v-card rounded="xl" variant="outlined" class="mb-4">
        <v-card-title class="d-flex flex-wrap align-center ga-2">
          <span class="d-flex align-center ga-2">
            <v-icon color="primary">mdi-chart-timeline-variant</v-icon>
            Consumo geral dos containers
          </span>
          <v-spacer></v-spacer>
          <v-chip color="primary" size="small" variant="tonal">
            CPU {{ latestAggregate?.cpuPercent.toFixed(1) ?? '0.0' }}%
          </v-chip>
          <v-chip color="secondary" size="small" variant="tonal">
            RAM {{ latestAggregate?.memoryPercent.toFixed(1) ?? '0.0' }}%
          </v-chip>
        </v-card-title>
        <v-card-subtitle>
          Histórico desta sessão, atualizado automaticamente enquanto o SSE estiver conectado.
        </v-card-subtitle>
        <v-card-text>
          <BaseMetricChart :series="aggregateSeries" unit-type="percentage"></BaseMetricChart>
        </v-card-text>
      </v-card>

      <v-row>
        <v-col cols="12" lg="5">
          <v-card rounded="xl" variant="outlined" height="100%">
            <v-card-title class="d-flex align-center ga-2">
              <v-icon color="primary">mdi-server-outline</v-icon>
              Docker Engine
            </v-card-title>
            <v-list density="compact">
              <v-list-item title="Host" :subtitle="docker.status?.name || '—'"></v-list-item>
              <v-list-item
                title="Versão / API"
                :subtitle="`${docker.status?.engineVersion || '—'} / ${docker.status?.apiVersion || '—'}`"
              ></v-list-item>
              <v-list-item
                title="Sistema"
                :subtitle="`${docker.status?.operatingSystem || '—'} · ${docker.status?.architecture || '—'}`"
              ></v-list-item>
              <v-list-item
                title="Capacidade"
                :subtitle="`${docker.status?.cpus ?? 0} CPUs · ${formatBinaryBytes(docker.status?.memoryTotalBytes)}`"
              ></v-list-item>
            </v-list>
          </v-card>
        </v-col>
        <v-col cols="12" lg="7">
          <v-card rounded="xl" variant="outlined" height="100%">
            <v-card-title class="d-flex align-center justify-space-between ga-2">
              <span class="d-flex align-center ga-2">
                <v-icon color="primary">mdi-chart-box-outline</v-icon>
                Containers em execução
              </span>
              <span class="text-caption text-medium-emphasis">
                {{
                  docker.metrics?.collectedAt ? formatRelativeTime(docker.metrics.collectedAt) : ''
                }}
              </span>
            </v-card-title>
            <v-table density="compact">
              <thead>
                <tr>
                  <th>Container</th>
                  <th class="text-right">CPU</th>
                  <th class="text-right">Memória</th>
                  <th class="text-right">Rede</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="metric in topMetrics" :key="metric.containerId">
                  <td>
                    <div class="font-weight-medium">{{ metric.containerName }}</div>
                    <div class="text-caption text-medium-emphasis">
                      {{ metric.projectName || 'Avulso' }}
                    </div>
                  </td>
                  <td class="text-right">{{ metric.cpu.usagePercent.toFixed(2) }}%</td>
                  <td class="text-right">
                    {{ metric.memory.usagePercent.toFixed(2) }}%
                    <div class="text-caption">
                      {{ formatBinaryBytes(metric.memory.usageBytes) }}
                    </div>
                  </td>
                  <td class="text-right">
                    ↓ {{ formatDecimalBytes(metric.network.receivedBytes) }}
                    <div class="text-caption">
                      ↑ {{ formatDecimalBytes(metric.network.transmittedBytes) }}
                    </div>
                  </td>
                </tr>
                <tr v-if="topMetrics.length === 0">
                  <td colspan="4" class="text-center text-medium-emphasis py-8">
                    Nenhum container em execução.
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-card>
        </v-col>
      </v-row>
    </template>

    <v-skeleton-loader v-else-if="docker.loading" type="card, card" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import PageHeader from '@/components/PageHeader.vue'
import { useDockerStore } from '@/stores/docker'
import { formatBinaryBytes, formatDecimalBytes, formatRelativeTime } from '@/utils/formatters'

const docker = useDockerStore()

const summaryCards = computed(() => [
  {
    label: 'Containers ativos',
    value: docker.runningContainers,
    icon: 'mdi-play-circle-outline',
    color: 'success',
  },
  {
    label: 'Containers parados',
    value: docker.stoppedContainers,
    icon: 'mdi-stop-circle-outline',
    color: 'warning',
  },
  {
    label: 'Volumes',
    value: docker.volumes.length,
    icon: 'mdi-database-outline',
    color: 'info',
  },
  {
    label: 'Imagens',
    value: docker.images.length,
    icon: 'mdi-layers-outline',
    color: 'primary',
  },
])

const topMetrics = computed(() =>
  [...(docker.metrics?.containers ?? [])]
    .sort(
      (left, right) =>
        right.cpu.usagePercent +
        right.memory.usagePercent -
        (left.cpu.usagePercent + left.memory.usagePercent)
    )
    .slice(0, 8)
)

const latestAggregate = computed(() => docker.aggregateHistory.at(-1) ?? null)

const aggregateSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'docker-cpu-total',
    label: 'CPU total',
    color: '#2196F3',
    fillArea: true,
    data: docker.aggregateHistory.map((sample) => ({
      time: sample.recordedAt,
      value: sample.cpuPercent,
    })),
  },
  {
    id: 'docker-memory-total',
    label: 'RAM dos containers / capacidade da Engine',
    color: '#7E57C2',
    fillArea: false,
    data: docker.aggregateHistory.map((sample) => ({
      time: sample.recordedAt,
      value: sample.memoryPercent,
    })),
  },
])
</script>
