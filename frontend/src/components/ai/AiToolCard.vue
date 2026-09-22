<template>
  <v-card variant="outlined" density="compact" class="my-2 rounded-lg tool-card" :color="cardColor">
    <div
      class="pa-2 d-flex align-center justify-space-between cursor-pointer"
      @click="expanded = !expanded"
    >
      <div class="d-flex align-center ga-2 min-w-0">
        <v-avatar size="28" :color="meta.color" variant="tonal">
          <v-icon size="16">{{ meta.icon }}</v-icon>
        </v-avatar>
        <div class="min-w-0">
          <div class="text-caption font-weight-bold">{{ meta.label }}</div>
          <div class="text-caption text-grey text-truncate max-w-300">
            {{ formatToolArgs(tool.arguments) }}
          </div>
        </div>
      </div>

      <div class="d-flex align-center ga-1">
        <v-chip size="x-small" :color="status.color" variant="tonal" class="font-weight-medium">
          <v-progress-circular
            v-if="tool.status === 'running'"
            indeterminate
            size="10"
            width="2"
            class="mr-1"
          />
          <v-icon v-else start size="12">{{ status.icon }}</v-icon>
          {{ status.label }}
        </v-chip>

        <v-btn icon size="x-small" variant="text" :color="cardColor">
          <v-icon size="16">{{ expanded ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
        </v-btn>
      </div>
    </div>

    <!-- Pedido de confirmação: a IA propôs, o usuário decide -->
    <div v-if="tool.summary" class="px-3 pb-3">
      <div class="text-body-2 mb-2 d-flex align-start ga-2">
        <v-icon size="18" :color="tool.status === 'awaiting' ? 'warning' : status.color">
          mdi-hand-back-right-outline
        </v-icon>
        <span>{{ tool.summary }}</span>
      </div>
      <div v-if="tool.status === 'awaiting'" class="d-flex ga-2 flex-wrap">
        <v-btn
          size="small"
          color="primary"
          variant="flat"
          prepend-icon="mdi-check"
          @click.stop="aiStore.confirmTool(messageId, tool.id)"
        >
          Confirmar
        </v-btn>
        <v-btn
          size="small"
          color="error"
          variant="outlined"
          prepend-icon="mdi-close"
          @click.stop="aiStore.cancelTool(messageId, tool.id)"
        >
          Cancelar
        </v-btn>
      </div>
      <div v-else-if="tool.status === 'error' && errorText" class="text-caption text-error">
        {{ errorText }}
      </div>
    </div>

    <div v-if="tool.chart" class="px-2 pb-2">
      <AiToolChart :chart="tool.chart" />
    </div>

    <v-expand-transition>
      <div v-if="expanded" class="pa-3 pt-0 border-t mt-1">
        <div class="text-caption font-weight-bold text-grey-darken-1 mb-1">Parâmetros:</div>
        <pre class="bg-grey-lighten-4 pa-2 rounded text-caption font-mono mb-2 overflow-x-auto">{{
          JSON.stringify(tool.arguments, null, 2)
        }}</pre>

        <div v-if="tool.result" class="text-caption font-weight-bold text-grey-darken-1 mb-1">
          Resultado:
        </div>
        <pre
          v-if="tool.result"
          class="bg-grey-lighten-4 pa-2 rounded text-caption font-mono overflow-x-auto max-h-200"
          >{{ JSON.stringify(tool.result, null, 2) }}</pre>
      </div>
    </v-expand-transition>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useAiStore, type AiToolCallState } from '@/stores/ai'
import type { AiToolStatus } from '@/utils/aiChatStream'
import AiToolChart from './AiToolChart.vue'
import { aiToolMeta, formatToolArgs } from './aiToolMeta'

const props = defineProps<{
  tool: AiToolCallState
  messageId: string
}>()

const aiStore = useAiStore()
const expanded = ref(false)

const meta = computed(() => aiToolMeta(props.tool.name))

interface StatusPresentation {
  label: string
  icon: string
  color: string
}

const STATUS: Record<AiToolStatus, StatusPresentation> = {
  running: { label: 'Executando', icon: 'mdi-progress-clock', color: 'primary' },
  done: { label: 'Concluído', icon: 'mdi-check', color: 'success' },
  error: { label: 'Falha', icon: 'mdi-alert-circle', color: 'error' },
  awaiting: { label: 'Aguardando você', icon: 'mdi-account-question', color: 'warning' },
  cancelled: { label: 'Cancelado', icon: 'mdi-cancel', color: 'grey' },
}

const status = computed(() => STATUS[props.tool.status])

const cardColor = computed(() => {
  if (props.tool.status === 'awaiting') return 'warning'
  if (props.tool.status === 'running') return 'primary'
  if (props.tool.status === 'error') return 'error'
  return 'grey-lighten-1'
})

const errorText = computed(() => {
  const error = props.tool.result?.error
  return typeof error === 'string' ? error : null
})
</script>

<style scoped>
.tool-card {
  background-color: rgba(var(--v-theme-surface), 0.9);
  transition: border-color 0.2s ease;
}
.max-w-300 {
  max-width: 300px;
}
.max-h-200 {
  max-height: 200px;
}
.min-w-0 {
  min-width: 0;
}
</style>
