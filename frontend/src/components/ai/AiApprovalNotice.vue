<template>
  <v-snackbar
    :model-value="Boolean(approval)"
    timeout="-1"
    location="bottom end"
    color="surface"
    variant="elevated"
    multi-line
    vertical
    class="ai-approval-notice"
  >
    <template v-if="approval">
      <div class="d-flex align-center ga-2 mb-2 flex-wrap">
        <v-icon color="warning">mdi-robot-outline</v-icon>
        <span class="font-weight-bold">Laya aguarda sua aprovação</span>
        <v-chip size="x-small" :color="risk.color" variant="flat">
          <v-icon start size="12">{{ risk.icon }}</v-icon>
          {{ risk.label }}
        </v-chip>
      </div>
      <div class="text-body-medium summary">{{ approval.summary }}</div>
      <div class="text-body-small mt-1">{{ approval.reason }}</div>
      <div class="text-body-small mt-1">
        Pedido {{ formatRelativeTime(approval.createdAt) }}.
        <span v-if="queued > 0">{{ queued }} outro(s) na fila.</span>
      </div>
    </template>

    <template #actions>
      <v-btn
        v-if="approval && aiStore.hasTool(approval.callId)"
        color="primary"
        variant="text"
        prepend-icon="mdi-chat-outline"
        @click="aiStore.isDrawerOpen = true"
      >
        Ver no chat
      </v-btn>
      <v-btn
        color="error"
        variant="outlined"
        prepend-icon="mdi-close"
        :disabled="busy"
        @click="dismiss"
      >
        Cancelar
      </v-btn>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-check"
        :loading="busy"
        @click="confirm"
      >
        Confirmar
      </v-btn>
    </template>
  </v-snackbar>

  <v-snackbar
    v-model="feedback.visible"
    :color="feedback.color"
    timeout="4000"
    location="bottom end"
  >
    {{ feedback.message }}
  </v-snackbar>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useAiStore } from '@/stores/ai'
import { useAiApprovalsStore } from '@/stores/aiApprovals'
import { aiRiskPresentation } from '@/utils/aiRisk'
import { formatRelativeTime } from '@/utils/formatters'

const aiStore = useAiStore()
const approvals = useAiApprovalsStore()
const route = useRoute()
const busy = ref(false)
const feedback = reactive({ visible: false, message: '', color: 'success' })

/** O chat está na tela: o card da conversa já mostra o pedido que ela tem. */
const chatVisible = computed(() => aiStore.isDrawerOpen || route.name === 'ai-chat')

/** Pedidos que nenhum card visível mostra agora. */
const unseen = computed(() =>
  approvals.mine.filter((item) => !(chatVisible.value && aiStore.hasTool(item.callId)))
)
const approval = computed(() => unseen.value[0] ?? null)
const queued = computed(() => Math.max(unseen.value.length - 1, 0))
const risk = computed(() => aiRiskPresentation(approval.value?.risk ?? 'high'))

function notify(message: string, color: 'success' | 'error') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : 'Falha ao executar a ação'
}

async function confirm() {
  const current = approval.value
  if (!current) return
  busy.value = true
  try {
    const response = await approvals.confirm(current.callId)
    aiStore.applyApprovalDecision({ callId: current.callId, decision: 'confirmed' }, response)
    const failure = response.result?.error
    if (typeof failure === 'string') notify(`A ação falhou: ${failure}`, 'error')
    else notify('Ação executada.', 'success')
  } catch (err: unknown) {
    notify(messageOf(err), 'error')
  } finally {
    busy.value = false
  }
}

async function dismiss() {
  const current = approval.value
  if (!current) return
  aiStore.applyApprovalDecision({ callId: current.callId, decision: 'dismissed' })
  try {
    await approvals.dismiss(current.callId)
  } catch {
    // Sem pendência no servidor (reinício, expirou): já saiu da fila.
  }
}
</script>

<style scoped>
.ai-approval-notice :deep(.v-snackbar__wrapper) {
  border: 1px solid rgb(var(--v-theme-warning));
  color: rgb(var(--v-theme-on-surface));
  max-width: 460px;
}
.summary {
  white-space: pre-line;
}
</style>
