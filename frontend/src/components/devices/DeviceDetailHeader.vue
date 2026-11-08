<template>
  <v-card elevation="2" class="rounded-lg mb-3 mb-md-6">
    <!-- Identidade e ações principais -->
    <div
      class="d-flex flex-column flex-md-row align-stretch align-md-center ga-3 ga-md-4 pa-3 pa-md-4"
    >
      <div class="d-flex align-center ga-3 flex-grow-1 min-w-0">
        <v-avatar :color="typeMeta.color" size="48" class="flex-shrink-0">
          <v-icon color="white">{{ typeMeta.icon }}</v-icon>
        </v-avatar>
        <div class="min-w-0">
          <div class="d-flex align-center ga-2 flex-wrap">
            <h1 class="text-h6 text-md-h5 font-weight-bold text-break ma-0">{{ title }}</h1>
            <v-chip :color="getStatusColor(device?.status)" size="small" variant="flat">
              <v-icon start size="12">mdi-circle</v-icon>
              {{ (device?.status || 'unknown').toUpperCase() }}
            </v-chip>
          </div>
          <div class="d-flex flex-wrap ga-2 mt-2">
            <v-chip size="small" variant="tonal" color="primary" prepend-icon="mdi-ip-network">
              {{ device?.ipAddress || 'IP não informado' }}
            </v-chip>
            <v-chip
              size="small"
              variant="tonal"
              :color="typeMeta.color"
              :prepend-icon="typeMeta.icon"
            >
              {{ typeMeta.label }}
            </v-chip>
            <v-chip
              v-if="device?.vendor"
              size="small"
              variant="tonal"
              color="info"
              prepend-icon="mdi-factory"
            >
              {{ device.vendor }}
            </v-chip>
          </div>
        </div>
      </div>

      <div class="device-header__primary d-flex flex-wrap ga-2">
        <v-btn
          v-if="canCreateMonitor"
          color="primary"
          variant="flat"
          prepend-icon="mdi-plus"
          @click="emit('createMonitor')"
        >
          Novo monitor
        </v-btn>
        <AiAskButton :prompt="aiPrompt" :context="aiContext" size="default" variant="tonal" />
        <v-btn
          v-if="canEdit"
          color="primary"
          variant="outlined"
          prepend-icon="mdi-pencil"
          @click="emit('edit')"
        >
          Editar
        </v-btn>
      </div>
    </div>

    <!-- Ferramentas: agrupadas pelo propósito, quebram linha em vez de vazar -->
    <template v-if="toolGroups.length">
      <v-divider />
      <div class="device-tools pa-3 pa-md-4">
        <section
          v-for="group in toolGroups"
          :key="group.key"
          class="device-tools__group"
          :aria-label="group.title"
        >
          <div class="device-tools__title text-caption font-weight-bold text-primary mb-2">
            <v-icon size="16" start>{{ group.icon }}</v-icon>
            {{ group.title }}
          </div>
          <div class="device-tools__buttons">
            <v-btn
              v-for="tool in group.tools"
              :key="tool.key"
              color="primary"
              variant="tonal"
              :prepend-icon="tool.icon"
              :loading="busy[tool.key] === true"
              @click="emit('tool', tool.key)"
            >
              {{ tool.label }}
              <v-tooltip activator="parent" location="bottom" max-width="300">
                {{ tool.hint }}
              </v-tooltip>
            </v-btn>
          </div>
        </section>
      </div>
    </template>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import AiAskButton from '@/components/ai/AiAskButton.vue'
import type { Device } from '@/stores/devices'
import type { AiChatContext } from '@/stores/ai'
import { deviceTypeMeta } from '@/utils/deviceTypes'
import { getStatusColor } from '@/utils/monitorPresentation'
import type { DeviceToolGroup, DeviceToolKey } from '@/utils/deviceTools'

/**
 * Cabeçalho de `/devices/{id}`: quem é o equipamento, as ações principais e as
 * ferramentas. Não abre diálogo nenhum — avisa a página, que é a dona deles.
 */
const props = withDefaults(
  defineProps<{
    device: Device | null
    deviceId: number
    toolGroups: DeviceToolGroup[]
    busy?: Partial<Record<DeviceToolKey, boolean>>
    canCreateMonitor: boolean
    canEdit: boolean
    aiPrompt: string
    aiContext: AiChatContext
  }>(),
  { busy: () => ({}) }
)

const emit = defineEmits<{
  tool: [key: DeviceToolKey]
  createMonitor: []
  edit: []
}>()

const title = computed(() => props.device?.name || `Dispositivo #${props.deviceId}`)
const typeMeta = computed(() => deviceTypeMeta(props.device?.type))
</script>

<style scoped>
.device-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 16px 40px;
}

.device-tools__title {
  display: flex;
  align-items: center;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.device-tools__buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

/* Celular: botões grandes, em grade de duas colunas, sem nada cortado. */
@media (max-width: 599.98px) {
  .device-header__primary > * {
    flex: 1 1 0;
  }

  .device-tools__group {
    width: 100%;
  }

  .device-tools__buttons {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .device-tools__buttons :deep(.v-btn) {
    min-width: 0;
  }
}
</style>
