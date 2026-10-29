<template>
  <DashboardCard
    title="Docker"
    icon="mdi-docker"
    :subtitle="`${docker.status?.name || 'Engine ativa'} · tempo real`"
    to="/docker"
    to-label="Abrir Docker"
  >
    <template #actions>
      <v-chip color="success" size="small" variant="tonal" prepend-icon="mdi-play-circle-outline">
        {{ docker.runningContainers }} ativos
      </v-chip>
      <v-chip
        v-if="docker.stoppedContainers > 0"
        color="warning"
        size="small"
        variant="tonal"
        prepend-icon="mdi-stop-circle-outline"
      >
        {{ docker.stoppedContainers }} parados
      </v-chip>
    </template>

    <v-row dense>
      <v-col cols="12" md="4">
        <div class="docker-metric">
          <div class="docker-metric__head">
            <span>CPU dos containers</span>
            <strong>{{ formatPercent(current?.cpuPercent ?? 0) }}</strong>
          </div>
          <MonitorSparkline
            :data="cpuHistory"
            :color="paletteColor(0)"
            width="100%"
            :height="48"
          ></MonitorSparkline>
        </div>
      </v-col>
      <v-col cols="12" md="4">
        <div class="docker-metric">
          <div class="docker-metric__head">
            <span>RAM dos containers</span>
            <strong>{{ memorySummary }}</strong>
          </div>
          <MonitorSparkline
            :data="memoryHistory"
            :color="paletteColor(6)"
            width="100%"
            :height="48"
            unit="bytes"
          ></MonitorSparkline>
        </div>
      </v-col>
      <v-col cols="12" md="4">
        <div class="docker-metric">
          <div class="docker-metric__head">
            <span>Tráfego acumulado</span>
          </div>
          <div class="docker-metric__row">
            <span><v-icon size="16" color="success">mdi-arrow-down</v-icon> Recebido</span>
            <strong>{{ formatDecimalBytes(current?.networkReceivedBytes) }}</strong>
          </div>
          <div class="docker-metric__row">
            <span><v-icon size="16" color="info">mdi-arrow-up</v-icon> Enviado</span>
            <strong>{{ formatDecimalBytes(current?.networkTransmittedBytes) }}</strong>
          </div>
        </div>
      </v-col>
    </v-row>
  </DashboardCard>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import MonitorSparkline, { type SparklinePoint } from '@/components/MonitorSparkline.vue'
import { useDockerStore } from '@/stores/docker'
import { paletteColor } from '@/utils/charts/scale'
import { formatBinaryBytes, formatDecimalBytes, formatPercent } from '@/utils/formatters'

const dockerStore = useDockerStore()
/**
 * O dashboard é da central: sempre o Docker dela (o agente local), qualquer
 * que seja o host aberto na tela Docker. Sem Engine disponível o widget nem
 * entra na grade (ver `widgetRegistry`).
 */
const docker = computed(() => {
  const local = dockerStore.localView
  const running = local.containers.filter((container) => container.state === 'running').length
  return {
    status: local.status,
    runningContainers: running,
    stoppedContainers: local.containers.length - running,
    aggregateHistory: local.aggregateHistory,
  }
})
const current = computed(() => docker.value.aggregateHistory.at(-1) ?? null)
const memorySummary = computed(() => {
  if (!current.value) return '0 B'
  const used = formatBinaryBytes(current.value.memoryUsageBytes, { fractionDigits: 1 })
  const total = formatBinaryBytes(docker.value.status?.memoryTotalBytes, {
    fractionDigits: 1,
    fallback: 'N/D',
  })
  return `${used} / ${total}`
})
const cpuHistory = computed<SparklinePoint[]>(() =>
  docker.value.aggregateHistory.map((sample) => ({
    value: sample.cpuPercent,
    recordedAt: sample.recordedAt,
  }))
)
const memoryHistory = computed<SparklinePoint[]>(() =>
  docker.value.aggregateHistory.map((sample) => ({
    value: sample.memoryUsageBytes,
    recordedAt: sample.recordedAt,
  }))
)
</script>

<style scoped>
.docker-metric {
  height: 100%;
  min-height: 96px;
  padding: 12px 14px;
  border-radius: 10px;
  background: rgba(var(--v-theme-primary), 0.05);
}

.docker-metric__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
  font-size: 0.8rem;
  font-weight: 600;
}

.docker-metric__head strong,
.docker-metric__row strong {
  font-variant-numeric: tabular-nums;
}

.docker-metric__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 6px;
  font-size: 0.85rem;
}
</style>
