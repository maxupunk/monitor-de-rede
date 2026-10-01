<template>
  <v-dialog :model-value="Boolean(approval)" max-width="560" persistent>
    <v-card v-if="approval" class="rounded-lg" :color="isWrite ? 'error' : undefined">
      <v-card-title class="d-flex align-center ga-2 text-wrap">
        <v-icon :color="isWrite ? undefined : 'warning'">mdi-hand-back-right-outline</v-icon>
        Aprovar acesso ao equipamento?
      </v-card-title>
      <v-card-subtitle class="text-wrap">
        {{ approval.pluginName }} · ação “{{ approval.action }}” · {{ approval.deviceName }}
      </v-card-subtitle>

      <v-card-text>
        <v-sheet rounded="lg" border class="pa-3 mb-3 request-sheet">
          <div class="d-flex align-center ga-2 mb-2 flex-wrap">
            <v-chip size="small" color="primary" variant="flat">
              {{ approval.request.transport.toUpperCase() }}
            </v-chip>
            <v-chip size="small" :color="effect.color" variant="flat">
              <v-icon start size="14">{{ effect.icon }}</v-icon>
              {{ effect.label }}
            </v-chip>
          </div>
          <code class="d-block font-mono text-body-2 request">{{ approval.request.summary }}</code>
        </v-sheet>

        <div v-if="approval.request.reason" class="text-body-2 mb-3">
          <strong>Motivo:</strong> {{ approval.request.reason }}
        </div>

        <v-alert
          :type="isWrite ? 'error' : 'warning'"
          :variant="isWrite ? 'flat' : 'tonal'"
          density="compact"
          class="mb-2"
        >
          {{
            isWrite
              ? 'Este comando ALTERA o equipamento. Uma IA ou um script pode errar — revise antes de aprovar.'
              : 'Só leitura, mas é um acesso real ao equipamento.'
          }}
        </v-alert>
        <div class="text-body-small">
          Expira {{ formatRelativeTime(approval.expiresAt) }}.
          <span v-if="store.pendingApprovals.length > 1">
            {{ store.pendingApprovals.length - 1 }} pedido(s) na fila.
          </span>
        </div>
      </v-card-text>

      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn
          :color="isWrite ? undefined : 'error'"
          variant="outlined"
          prepend-icon="mdi-close"
          :disabled="busy"
          @click="answer(false)"
        >
          Negar
        </v-btn>
        <v-btn
          :color="isWrite ? 'surface' : 'primary'"
          variant="flat"
          prepend-icon="mdi-check"
          :loading="busy"
          @click="answer(true)"
        >
          Aprovar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { usePluginsStore } from '@/stores/plugins'
import { formatRelativeTime } from '@/utils/formatters'
import { effectPresentation } from '@/utils/pluginPresentation'

const store = usePluginsStore()
const busy = ref(false)

const approval = computed(() => store.nextApproval)
const isWrite = computed(() => approval.value?.request.effect === 'write')
const effect = computed(() => effectPresentation(approval.value?.request.effect ?? 'read'))

async function answer(approved: boolean) {
  const current = approval.value
  if (!current) return
  busy.value = true
  try {
    await store.respondApproval(current.key, approved)
  } catch {
    // Pedido expirado ou já respondido em outra aba: some da fila de qualquer jeito.
  } finally {
    busy.value = false
  }
}
</script>

<style scoped>
.request-sheet {
  background-color: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
}
.request {
  word-break: break-all;
  white-space: pre-wrap;
}
</style>
