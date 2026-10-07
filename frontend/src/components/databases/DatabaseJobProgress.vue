<template>
  <div class="job pa-3 rounded-lg" :class="`job--${job.status}`">
    <div class="d-flex align-center ga-2 mb-1">
      <v-progress-circular
        v-if="job.status === 'running'"
        indeterminate
        size="16"
        width="2"
        color="primary"
      />
      <v-icon v-else size="18" :color="job.status === 'success' ? 'success' : 'error'">
        {{ job.status === 'success' ? 'mdi-check-circle' : 'mdi-alert-circle' }}
      </v-icon>
      <span class="font-weight-bold text-body-2">{{ title }}</span>
      <v-spacer></v-spacer>
      <span v-if="job.databasesTotal > 1" class="text-caption text-high-emphasis">
        {{ Math.min(job.databasesDone + (job.status === 'running' ? 1 : 0), job.databasesTotal) }}
        de {{ job.databasesTotal }} bancos
      </span>
    </div>

    <template v-if="job.status === 'running'">
      <div class="text-body-2 text-truncate" :title="job.stage">
        <span v-if="job.database" class="font-weight-medium">{{ job.database }} · </span>
        {{ job.stage }}
      </div>
      <div class="text-caption text-high-emphasis">
        {{ formatCompactCount(job.rows) }} linhas · {{ formatBytes(job.bytes) }}
      </div>
      <v-progress-linear
        :model-value="overall"
        :indeterminate="job.databasesTotal <= 1"
        color="primary"
        rounded
        class="mt-2"
      />
    </template>
    <div v-else class="text-body-2 message">{{ job.message }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DatabaseJobSnapshot } from '@/bindings/DatabaseJobSnapshot'
import { formatBytes, formatCompactCount } from '@/utils/formatters'

/** Andamento ao vivo de um backup ou de uma restauração (vem do SSE). */
const props = defineProps<{ job: DatabaseJobSnapshot }>()

const title = computed(() => {
  const what = props.job.kind === 'backup' ? 'Backup' : 'Restauração'
  if (props.job.status === 'running') return `${what} em andamento`
  return props.job.status === 'success' ? `${what} concluído` : `${what} falhou`
})

/** Fração dos bancos concluídos, quando há mais de um. */
const overall = computed(() =>
  props.job.databasesTotal > 0 ? (props.job.databasesDone / props.job.databasesTotal) * 100 : 0
)
</script>

<style scoped>
.job {
  border: 1px solid rgba(var(--v-theme-primary), 0.4);
  background: rgba(var(--v-theme-primary), 0.06);
}
.job--success {
  border-color: rgba(var(--v-theme-success), 0.5);
  background: rgba(var(--v-theme-success), 0.07);
}
.job--failed {
  border-color: rgba(var(--v-theme-error), 0.5);
  background: rgba(var(--v-theme-error), 0.07);
}
.message {
  word-break: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
