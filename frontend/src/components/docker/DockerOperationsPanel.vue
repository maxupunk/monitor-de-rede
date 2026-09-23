<template>
  <div v-if="visibleOperations.length > 0" class="d-flex flex-column ga-2">
    <v-card
      v-for="operation in visibleOperations"
      :key="operation.operationId"
      rounded="lg"
      variant="tonal"
      :color="stateColor(operation.state)"
    >
      <v-card-text class="d-flex align-start ga-3 py-3">
        <v-progress-circular
          v-if="operation.state === 'running'"
          indeterminate
          size="20"
          width="2"
        ></v-progress-circular>
        <v-icon v-else>{{ stateIcon(operation.state) }}</v-icon>
        <div class="flex-grow-1 operation-text">
          <div class="text-body-2 font-weight-medium">
            {{ kindLabel(operation.kind) }} · {{ operation.target }}
            <span class="text-medium-emphasis">({{ hostName(operation.hostKey) }})</span>
          </div>
          <div class="text-caption operation-line">
            {{ operation.message || operation.lines.at(-1) || 'Aguardando o host…' }}
          </div>
        </div>
        <v-btn
          v-if="operation.state !== 'running'"
          icon="mdi-close"
          size="small"
          variant="text"
          aria-label="Dispensar"
          @click="docker.dismissOperation(operation.operationId)"
        ></v-btn>
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDockerStore } from '@/stores/docker'

const docker = useDockerStore()

/** Só as operações do host em tela: as de outros hosts seguem no store. */
const visibleOperations = computed(() =>
  docker.operations.filter((operation) => operation.hostKey === docker.selectedHostKey)
)

const KIND_LABELS: Record<string, string> = {
  pull: 'Pull de imagem',
  update: 'Atualização de container',
  compose: 'Compose',
}

function kindLabel(kind: string): string {
  return KIND_LABELS[kind] ?? kind
}

function hostName(hostKey: string): string {
  return docker.hosts.find((host) => host.key === hostKey)?.name ?? hostKey
}

function stateColor(state: string): string {
  if (state === 'succeeded') return 'success'
  if (state === 'failed') return 'error'
  return 'info'
}

function stateIcon(state: string): string {
  return state === 'succeeded' ? 'mdi-check-circle-outline' : 'mdi-alert-circle-outline'
}
</script>

<style scoped>
.operation-text {
  min-width: 0;
}

.operation-line {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  overflow-wrap: anywhere;
}
</style>
