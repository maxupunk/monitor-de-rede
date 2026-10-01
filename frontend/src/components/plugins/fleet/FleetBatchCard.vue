<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-2 mb-3">
      <v-chip :color="status.color" size="small" variant="flat">
        <v-icon start size="14">{{ status.icon }}</v-icon>
        {{ status.label }}
      </v-chip>
      <span class="text-body-2">
        {{ done }} de {{ batch.devices.length }} equipamento(s) ·
        {{ formatDateTime(batch.createdAt) }}
      </span>
      <v-spacer></v-spacer>
      <v-btn
        v-if="batch.status === 'running' && canWrite"
        size="small"
        color="error"
        variant="outlined"
        prepend-icon="mdi-stop"
        @click="emit('cancel', batch)"
      >
        Cancelar
      </v-btn>
    </div>
    <v-progress-linear
      v-if="batch.status === 'running'"
      :model-value="(done / Math.max(batch.devices.length, 1)) * 100"
      color="primary"
      height="6"
      rounded
      class="mb-3"
    ></v-progress-linear>
    <v-alert v-if="batch.error" type="error" variant="tonal" density="compact" class="mb-3">
      {{ batch.error }}
    </v-alert>

    <v-card v-if="batch.result !== null" border flat class="rounded-lg mb-3">
      <v-card-title class="d-flex align-center ga-2 text-subtitle-1 font-weight-bold">
        <v-icon color="primary">mdi-sigma</v-icon>
        Consolidado
        <v-spacer></v-spacer>
        <v-btn
          v-if="batch.hasPatch && canWrite"
          size="small"
          color="success"
          variant="flat"
          prepend-icon="mdi-check"
          @click="emit('applyPatch', batch)"
        >
          Aceitar sugestão
        </v-btn>
      </v-card-title>
      <v-card-text>
        <PluginReport :output="batch.result" :labels="resultLabels ?? labels" />
        <p v-if="batch.hasPatch" class="text-body-2 mt-3">
          Aceitar grava os ajustes sugeridos na configuração de cada equipamento. Nada muda nos
          roteadores até você aplicar a configuração.
        </p>
      </v-card-text>
    </v-card>

    <v-expansion-panels variant="accordion" multiple>
      <v-expansion-panel v-for="device in batch.devices" :key="device.deviceId">
        <v-expansion-panel-title>
          <div class="d-flex align-center ga-2 flex-grow-1">
            <v-icon :color="batchStatusPresentation(device.status).color" size="18">
              {{ batchStatusPresentation(device.status).icon }}
            </v-icon>
            <span class="font-weight-bold">{{ device.deviceName }}</span>
            <v-chip
              :color="batchStatusPresentation(device.status).color"
              size="x-small"
              variant="tonal"
            >
              {{ batchStatusPresentation(device.status).label }}
            </v-chip>
          </div>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <v-alert
            v-if="device.error"
            type="error"
            variant="tonal"
            density="compact"
            class="mb-3"
            :text="device.error"
          ></v-alert>
          <PluginOutput
            v-if="device.output !== null"
            :output="device.output"
            :kind="kind"
            :labels="labels"
          />
          <div v-else-if="!device.error" class="text-body-2">Sem resultado ainda.</div>
          <v-btn
            v-if="device.runId"
            size="small"
            color="primary"
            variant="text"
            prepend-icon="mdi-format-list-bulleted"
            class="mt-2"
            @click="emit('openRun', device)"
          >
            Ver acessos ao equipamento
          </v-btn>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { BatchDevice } from '@/bindings/BatchDevice'
import type { OutputKind } from '@/bindings/OutputKind'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import { formatDateTime } from '@/utils/formatters'
import { batchStatusPresentation } from '@/utils/pluginPresentation'
import PluginOutput from '../PluginOutput.vue'
import PluginReport from '../PluginReport.vue'

const props = defineProps<{
  batch: PluginBatchView
  /** Formato da saída de cada equipamento (o da ação de dispositivo). */
  kind?: OutputKind
  /** Títulos da saída de cada equipamento e do consolidado. */
  labels?: Record<string, string>
  resultLabels?: Record<string, string>
  canWrite: boolean
}>()

const emit = defineEmits<{
  cancel: [batch: PluginBatchView]
  applyPatch: [batch: PluginBatchView]
  openRun: [device: BatchDevice]
}>()

const status = computed(() => batchStatusPresentation(props.batch.status))
const done = computed(
  () =>
    props.batch.devices.filter(
      (device) => device.status !== 'pending' && device.status !== 'running'
    ).length
)
</script>
