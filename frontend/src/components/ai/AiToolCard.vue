<template>
  <v-card
    variant="outlined"
    density="compact"
    :class="['my-2 rounded-lg tool-card', { 'tool-card--idle': !cardColor }]"
    :color="cardColor"
  >
    <div
      class="tool-card__header pa-2 d-flex align-center justify-space-between cursor-pointer"
      role="button"
      tabindex="0"
      :aria-expanded="expanded"
      @click="toggle"
      @keydown.enter.self.prevent="toggle"
      @keydown.space.self.prevent="toggle"
    >
      <div class="d-flex align-center ga-2 min-w-0">
        <v-avatar size="28" :color="meta.color" variant="tonal">
          <v-icon size="16">{{ meta.icon }}</v-icon>
        </v-avatar>
        <div class="min-w-0">
          <div class="text-body-small font-weight-bold">{{ meta.label }}</div>
          <div class="text-body-small text-truncate max-w-300">
            {{ formatToolArgs(tool.arguments) }}
          </div>
        </div>
      </div>

      <div class="d-flex align-center ga-1">
        <v-chip
          v-if="tool.autoApproved"
          size="x-small"
          color="error"
          variant="flat"
          title="Executado sem confirmação: o modo “Aceitar automaticamente” está ligado"
        >
          <v-icon start size="12">mdi-lightning-bolt</v-icon>
          Automático
        </v-chip>
        <v-chip
          v-if="tool.status !== 'done'"
          size="x-small"
          :color="status.color"
          variant="tonal"
          class="font-weight-medium"
        >
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

        <v-icon size="16" :color="cardColor ?? 'primary'" aria-hidden="true">
          {{ expanded ? 'mdi-chevron-up' : 'mdi-chevron-down' }}
        </v-icon>
      </div>
    </div>

    <!-- Pedido de confirmação: a IA propôs, o usuário decide -->
    <div v-if="tool.summary" class="px-3 pb-3">
      <div class="text-body-medium mb-2 d-flex align-start ga-2">
        <v-icon size="18" :color="tool.status === 'awaiting' ? 'warning' : status.color">
          mdi-hand-back-right-outline
        </v-icon>
        <span class="summary-text">{{ tool.summary }}</span>
      </div>
      <v-alert
        v-if="deviceAccess && tool.status === 'awaiting'"
        :type="writes ? 'error' : 'warning'"
        :variant="writes ? 'flat' : 'tonal'"
        density="compact"
        class="mb-2 text-body-small"
      >
        {{
          writes
            ? 'A IA pode errar (alucinar). Este comando ALTERA a configuração do equipamento — revise antes de aprovar.'
            : 'Acesso real ao equipamento, só leitura. A IA pode interpretar errado o que ler.'
        }}
      </v-alert>
      <div v-if="activeTest && tool.status === 'awaiting'" class="text-body-small mb-2">
        Pedido porque os testes ativos estão em “Pedir permissão”.
        <router-link :to="{ name: 'settings', query: { tab: 'ia' } }" class="text-primary">
          Mudar nas configurações da IA
        </router-link>
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
      <div v-else-if="tool.status === 'error' && errorText" class="text-body-small text-error">
        {{ errorText }}
      </div>
    </div>

    <div v-if="tool.chart" class="px-2 pb-2">
      <AiToolChart :chart="tool.chart" />
    </div>

    <v-expand-transition>
      <div v-if="expanded" class="pa-3 pt-0 border-t mt-1">
        <div class="text-body-small font-weight-bold mb-1">Parâmetros:</div>
        <pre class="code-block pa-2 rounded text-body-small font-mono mb-2 overflow-x-auto">{{
          JSON.stringify(tool.arguments, null, 2)
        }}</pre>

        <div v-if="tool.result" class="text-body-small font-weight-bold mb-1">Resultado:</div>
        <pre
          v-if="tool.result"
          class="code-block pa-2 rounded text-body-small font-mono overflow-x-auto max-h-200"
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
import { aiToolMeta, formatToolArgs, isActiveTestTool, isDeviceAccessTool } from './aiToolMeta'

const props = defineProps<{
  tool: AiToolCallState
  messageId: string
}>()

const aiStore = useAiStore()
const expanded = ref(false)

function toggle() {
  expanded.value = !expanded.value
}

const meta = computed(() => aiToolMeta(props.tool.name))
const deviceAccess = computed(() => isDeviceAccessTool(props.tool.name))
const activeTest = computed(() => isActiveTestTool(props.tool.name))
/** O backend escreve "ALTERA O EQUIPAMENTO" no resumo quando o efeito é escrita. */
const writes = computed(() => (props.tool.summary ?? '').includes('ALTERA O EQUIPAMENTO'))

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
  cancelled: { label: 'Cancelado', icon: 'mdi-cancel', color: 'secondary' },
}

const status = computed(() => STATUS[props.tool.status])

/** Só o que pede atenção tinge o cartão; concluído e cancelado ficam na cor do tema. */
const cardColor = computed(() => {
  if (props.tool.status === 'awaiting') return 'warning'
  if (props.tool.status === 'running') return 'primary'
  if (props.tool.status === 'error') return 'error'
  return undefined
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
.tool-card--idle {
  border-color: rgba(var(--v-border-color), var(--v-border-opacity));
  color: rgb(var(--v-theme-on-surface));
}
.tool-card__header:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: -2px;
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
.summary-text {
  white-space: pre-line;
}
</style>
