<template>
  <v-card v-if="docker.available" rounded="xl" variant="outlined" class="docker-summary-card">
    <v-card-text class="pa-4 pa-md-5">
      <div class="d-flex flex-wrap align-center ga-3 mb-4">
        <v-avatar color="primary" variant="tonal" size="44">
          <v-icon size="26">mdi-docker</v-icon>
        </v-avatar>
        <div>
          <div class="text-subtitle-1 font-weight-bold">Docker</div>
          <div class="text-caption text-medium-emphasis">
            {{ docker.status?.name || 'Engine ativa' }} · monitoramento em tempo real
          </div>
        </div>
        <v-spacer></v-spacer>
        <v-chip color="success" size="small" variant="tonal" prepend-icon="mdi-circle-small">
          {{ docker.runningContainers }} ativos
        </v-chip>
        <v-chip v-if="docker.stoppedContainers > 0" color="warning" size="small" variant="tonal">
          {{ docker.stoppedContainers }} parados
        </v-chip>
        <v-btn
          to="/docker"
          variant="tonal"
          color="primary"
          size="small"
          append-icon="mdi-arrow-right"
        >
          Abrir Docker
        </v-btn>
      </div>

      <v-row dense>
        <v-col cols="12" md="4">
          <div class="docker-metric pa-3 rounded-lg">
            <div class="d-flex align-center justify-space-between ga-3 mb-2">
              <span class="text-caption text-medium-emphasis">CPU dos containers</span>
              <strong>{{ current?.cpuPercent.toFixed(1) ?? '0.0' }}%</strong>
            </div>
            <MonitorSparkline
              :data="cpuHistory"
              color="#2196F3"
              width="100%"
              :height="42"
            ></MonitorSparkline>
          </div>
        </v-col>
        <v-col cols="12" md="4">
          <div class="docker-metric pa-3 rounded-lg">
            <div class="d-flex align-center justify-space-between ga-3 mb-2">
              <span class="text-caption text-medium-emphasis">RAM dos containers / Engine</span>
              <strong>{{ current?.memoryPercent.toFixed(1) ?? '0.0' }}%</strong>
            </div>
            <MonitorSparkline
              :data="memoryHistory"
              color="#7E57C2"
              width="100%"
              :height="42"
            ></MonitorSparkline>
          </div>
        </v-col>
        <v-col cols="12" md="4">
          <div class="docker-metric pa-3 rounded-lg h-100 d-flex flex-column justify-center">
            <div class="text-caption text-medium-emphasis mb-2">Tráfego acumulado</div>
            <div class="d-flex align-center justify-space-between ga-2">
              <span><v-icon size="16" color="success">mdi-arrow-down</v-icon> Recebido</span>
              <strong>{{ formatDecimalBytes(current?.networkReceivedBytes) }}</strong>
            </div>
            <div class="d-flex align-center justify-space-between ga-2 mt-2">
              <span><v-icon size="16" color="info">mdi-arrow-up</v-icon> Enviado</span>
              <strong>{{ formatDecimalBytes(current?.networkTransmittedBytes) }}</strong>
            </div>
          </div>
        </v-col>
      </v-row>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import MonitorSparkline, { type SparklinePoint } from '@/components/MonitorSparkline.vue'
import { useDockerStore } from '@/stores/docker'
import { formatDecimalBytes } from '@/utils/formatters'

const docker = useDockerStore()
const current = computed(() => docker.aggregateHistory.at(-1) ?? null)
const cpuHistory = computed<SparklinePoint[]>(() =>
  docker.aggregateHistory.map((sample) => ({
    value: sample.cpuPercent,
    recordedAt: sample.recordedAt,
  }))
)
const memoryHistory = computed<SparklinePoint[]>(() =>
  docker.aggregateHistory.map((sample) => ({
    value: sample.memoryPercent,
    recordedAt: sample.recordedAt,
  }))
)
</script>

<style scoped>
.docker-summary-card {
  background:
    radial-gradient(circle at 95% 0%, rgba(var(--v-theme-primary), 0.12), transparent 34%),
    rgb(var(--v-theme-surface));
}

.docker-metric {
  background: rgba(var(--v-theme-primary), 0.045);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.08);
  min-height: 96px;
}
</style>
