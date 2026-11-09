<template>
  <v-card border flat class="pa-3" data-test="target-row">
    <div class="d-flex flex-wrap align-center ga-2">
      <div class="flex-grow-1 min-w-0">
        <div class="d-flex flex-wrap align-center ga-2">
          <span class="text-subtitle-1 font-weight-bold">{{ target.name }}</span>
          <v-chip
            :color="agentColor"
            size="small"
            variant="tonal"
            label
            :prepend-icon="target.agent?.connected ? 'mdi-lan-connect' : 'mdi-lan-disconnect'"
          >
            {{ target.agent?.name ?? 'Agente removido' }}
          </v-chip>
          <v-chip
            color="primary"
            size="small"
            variant="tonal"
            label
            prepend-icon="mdi-source-branch"
          >
            {{ target.reference }}
          </v-chip>
          <v-chip
            v-if="target.requiresApproval"
            color="warning"
            size="small"
            variant="tonal"
            label
            prepend-icon="mdi-account-check"
          >
            Com aprovação
          </v-chip>
          <v-chip
            v-if="windowText"
            color="info"
            size="small"
            variant="tonal"
            label
            prepend-icon="mdi-calendar-clock"
          >
            {{ windowText }}
          </v-chip>
          <v-chip v-if="!target.enabled" color="error" size="small" variant="tonal" label>
            Desativado
          </v-chip>
        </div>
        <div class="text-body-2 text-high-emphasis mt-1">
          <template v-if="target.live">
            No ar:
            <a
              :href="commitUrl(repository, target.live.commitSha)"
              target="_blank"
              rel="noopener"
              class="font-weight-bold text-primary"
              >{{ target.live.shortSha }}</a
            >
            {{ commitTitle(target.live.commitMessage) }}
            · {{ formatRelativeTime(target.live.finishedAt ?? target.live.createdAt) }}
          </template>
          <template v-else>Ainda sem versão publicada por aqui.</template>
          · projeto <code>{{ target.project }}</code>
        </div>
      </div>

      <div class="d-flex flex-wrap align-center ga-2">
        <v-btn
          v-if="target.pending"
          :color="pendingMeta.color"
          :variant="target.pending.status === 'awaiting_approval' ? 'flat' : 'tonal'"
          :prepend-icon="pendingMeta.icon"
          size="small"
          @click="emit('open-run', target.pending.id)"
        >
          {{ pendingLabel }}
        </v-btn>
        <template v-if="canAdmin">
          <v-btn
            color="success"
            variant="tonal"
            size="small"
            prepend-icon="mdi-rocket-launch"
            :disabled="!target.agent"
            @click="emit('publish', target)"
          >
            Publicar
          </v-btn>
          <v-btn
            color="primary"
            variant="tonal"
            size="small"
            prepend-icon="mdi-file-key"
            @click="emit('env', target)"
          >
            Variáveis
          </v-btn>
        </template>
        <v-btn
          color="primary"
          variant="text"
          size="small"
          prepend-icon="mdi-history"
          @click="emit('history', target)"
        >
          Histórico
        </v-btn>
        <v-menu v-if="canAdmin">
          <template #activator="{ props: menu }">
            <v-btn
              v-bind="menu"
              icon="mdi-dots-vertical"
              variant="text"
              color="primary"
              size="small"
              :aria-label="`Mais ações de ${target.name}`"
            ></v-btn>
          </template>
          <v-list density="compact">
            <v-list-item
              prepend-icon="mdi-pencil"
              title="Editar alvo"
              @click="emit('edit', target)"
            ></v-list-item>
            <v-list-item
              prepend-icon="mdi-delete"
              title="Remover alvo"
              base-color="error"
              @click="emit('remove', target)"
            ></v-list-item>
          </v-list>
        </v-menu>
      </div>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import { commitTitle, commitUrl, runStatusMeta, windowLabel } from '@/utils/deployPresentation'
import { formatRelativeTime, formatTimeUntil } from '@/utils/formatters'

const props = defineProps<{ target: DeployTargetView; repository: string; canAdmin: boolean }>()
const emit = defineEmits<{
  'open-run': [id: number]
  publish: [target: DeployTargetView]
  env: [target: DeployTargetView]
  history: [target: DeployTargetView]
  edit: [target: DeployTargetView]
  remove: [target: DeployTargetView]
}>()

const windowText = computed(() => windowLabel(props.target.windowStart, props.target.windowEnd))

const agentColor = computed(() => {
  const agent = props.target.agent
  if (!agent || !agent.connected) return 'error'
  return agent.allowsDeploy ? 'success' : 'warning'
})

const pendingMeta = computed(() => runStatusMeta(props.target.pending?.status ?? 'queued'))

/** O que o botão do deploy pendente diz: a ação esperada, não só o estado. */
const pendingLabel = computed(() => {
  const pending = props.target.pending
  if (!pending) return ''
  switch (pending.status) {
    case 'awaiting_approval':
      return `Revisar e publicar ${pending.shortSha}`
    case 'running':
      return `Publicando ${pending.shortSha}…`
    case 'scheduled':
      return `${pending.shortSha} ${formatTimeUntil(pending.scheduledFor)}`
    default:
      return `${pending.shortSha} na fila`
  }
})
</script>
