<template>
  <v-list density="comfortable" class="bg-transparent pa-0" data-test="runs-list">
    <v-list-item
      v-for="run in runs"
      :key="run.id"
      :title="title(run)"
      :subtitle="subtitle(run)"
      class="px-2 rounded-lg run-item"
      @click="emit('open', run.id)"
    >
      <template #prepend>
        <v-icon :color="runStatusMeta(run.status).color" class="mr-3">
          {{ runStatusMeta(run.status).icon }}
        </v-icon>
      </template>
      <template #append>
        <DeployRunStatusChip :status="run.status" />
      </template>
    </v-list-item>
    <div v-if="!runs.length" class="text-body-2 text-high-emphasis pa-2">
      {{ emptyText }}
    </div>
  </v-list>
</template>

<script setup lang="ts">
import DeployRunStatusChip from '@/components/deploys/DeployRunStatusChip.vue'
import type { DeployRunView } from '@/bindings/DeployRunView'
import { TRIGGER_LABEL, commitTitle, runStatusMeta } from '@/utils/deployPresentation'
import { formatElapsedMs, formatRelativeTime } from '@/utils/formatters'

const props = withDefaults(
  defineProps<{ runs: DeployRunView[]; showPlace?: boolean; emptyText?: string }>(),
  { showPlace: true, emptyText: 'Nenhum deploy ainda.' }
)
const emit = defineEmits<{ open: [id: number] }>()

function title(run: DeployRunView): string {
  const message = commitTitle(run.commitMessage)
  const place = props.showPlace ? `${run.applicationName} → ${run.targetName} · ` : ''
  return `${place}${run.shortSha}${message ? ` — ${message}` : ''}`
}

function subtitle(run: DeployRunView): string {
  const parts = [
    `#${run.id}`,
    `${TRIGGER_LABEL[run.trigger]}${run.triggeredBy ? ` (${run.triggeredBy})` : ''}`,
    formatRelativeTime(run.createdAt),
  ]
  if (run.durationMs !== null) parts.push(formatElapsedMs(run.durationMs))
  return parts.join(' · ')
}
</script>

<style scoped>
.run-item {
  cursor: pointer;
}
</style>
