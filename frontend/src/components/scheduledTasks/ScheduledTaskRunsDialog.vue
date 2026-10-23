<template>
  <v-dialog
    :model-value="modelValue"
    :fullscreen="smAndDown"
    max-width="860"
    scrollable
    @update:model-value="(open) => !open && close()"
  >
    <v-card v-if="task" :rounded="smAndDown ? 0 : 'lg'">
      <v-card-title class="d-flex align-center ga-2 pt-4">
        <v-icon color="primary">mdi-history</v-icon>
        <span class="flex-grow-1 text-truncate">Histórico · {{ task.name }}</span>
        <v-btn
          icon="mdi-close"
          variant="text"
          size="small"
          aria-label="Fechar"
          @click="close"
        ></v-btn>
      </v-card-title>
      <v-card-subtitle class="pb-3 subtitle">
        {{ task.scheduleLabel }} ·
        {{ task.enabled ? `próxima ${formatTimeUntil(task.nextRunAt)}` : 'pausada' }}
      </v-card-subtitle>
      <v-divider></v-divider>

      <v-card-text>
        <v-infinite-scroll :key="runs.scrollKey.value" empty-text="" @load="runs.load">
          <p v-if="runs.items.value.length === 0" class="text-body-medium text-center py-6 empty">
            Esta tarefa ainda não rodou.
          </p>
          <v-expansion-panels v-else variant="accordion" multiple>
            <v-expansion-panel v-for="run in runs.items.value" :key="run.id">
              <v-expansion-panel-title>
                <div class="d-flex align-center flex-wrap ga-2 w-100">
                  <ScheduledTaskStatusChip :status="run.status" size="x-small" />
                  <span class="font-weight-bold">{{ formatDateTime(run.startedAt) }}</span>
                  <v-chip size="x-small" color="info" variant="tonal" label>
                    {{ TRIGGER_LABEL[run.trigger] }}
                  </v-chip>
                  <span v-if="run.durationMs !== null" class="text-body-small">
                    {{ formatElapsedMs(run.durationMs) }}
                  </span>
                  <span v-if="run.message" class="text-body-small message text-truncate">
                    {{ run.message }}
                  </span>
                </div>
              </v-expansion-panel-title>
              <v-expansion-panel-text>
                <p v-if="run.message" class="text-body-medium mb-2">{{ run.message }}</p>
                <pre v-if="run.result !== null" class="result">{{ pretty(run.result) }}</pre>
                <p v-else class="text-body-small mb-0">Sem resultado gravado.</p>
              </v-expansion-panel-text>
            </v-expansion-panel>
          </v-expansion-panels>
        </v-infinite-scroll>
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Execuções de uma tarefa, da mais nova para a mais antiga, com o resultado
 * que a ferramenta devolveu. A notícia do SSE de uma execução desta tarefa
 * recomeça a lista.
 */
import { watch } from 'vue'
import { useDisplay } from 'vuetify'
import type { ScheduledTask } from '@/bindings/ScheduledTask'
import type { ScheduledTaskRun } from '@/bindings/ScheduledTaskRun'
import { useInfiniteList } from '@/composables/useInfiniteList'
import { useScheduledTasksStore } from '@/stores/scheduledTasks'
import { formatDateTime, formatElapsedMs, formatTimeUntil } from '@/utils/formatters'
import { TRIGGER_LABEL } from '@/utils/scheduledTasks/presentation'
import ScheduledTaskStatusChip from './ScheduledTaskStatusChip.vue'

const props = defineProps<{
  modelValue: boolean
  task: ScheduledTask | null
}>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const { smAndDown } = useDisplay()
const store = useScheduledTasksStore()

const runs = useInfiniteList<ScheduledTaskRun>(
  () => `/scheduled-tasks/${props.task?.id ?? 0}/runs`,
  { label: 'histórico da tarefa' }
)

watch(
  () => [props.modelValue, props.task?.id] as const,
  ([open]) => {
    if (open) runs.reset()
  }
)

watch(
  () => store.lastEvent,
  (event) => {
    if (props.modelValue && event && event.id === props.task?.id) runs.reset()
  }
)

function pretty(value: unknown): string {
  return JSON.stringify(value, null, 2)
}

function close() {
  emit('update:modelValue', false)
}
</script>

<style scoped>
.subtitle,
.empty,
.message {
  color: rgb(var(--v-theme-on-surface));
}
.message {
  max-width: 360px;
}
.result {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 320px;
  overflow: auto;
  padding: 8px;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 6px;
}
</style>
