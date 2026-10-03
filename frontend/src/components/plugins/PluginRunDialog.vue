<template>
  <v-dialog :model-value="modelValue" max-width="820" scrollable @update:model-value="close">
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2 flex-wrap">
        <v-icon color="primary">mdi-console</v-icon>
        {{ title }}
        <v-chip v-if="run" size="small" :color="status.color" variant="flat">
          <v-progress-circular
            v-if="run.status === 'running'"
            indeterminate
            size="12"
            width="2"
            class="mr-1"
          />
          <v-icon v-else start size="14">{{ status.icon }}</v-icon>
          {{ status.label }}
        </v-chip>
      </v-card-title>
      <v-card-subtitle v-if="run">{{ run.pluginName }} · {{ deviceName }}</v-card-subtitle>

      <v-card-text>
        <div v-if="!run" class="d-flex align-center ga-2 text-body-2">
          <v-progress-circular indeterminate size="18" width="2" color="primary" />
          Iniciando a execução…
        </div>
        <template v-else>
          <v-alert
            v-if="run.status === 'running'"
            type="info"
            variant="tonal"
            density="compact"
            class="mb-3"
          >
            Executando no equipamento. Se o plugin pedir aprovação, ela aparece nesta tela.
          </v-alert>
          <v-alert v-if="run.error" type="error" variant="tonal" class="mb-3">
            <div class="font-weight-bold mb-1">O equipamento respondeu com erro</div>
            <pre class="response font-mono text-body-small">{{ run.error }}</pre>
          </v-alert>
          <template v-if="run.output !== null && run.output !== undefined">
            <div class="text-subtitle-2 font-weight-bold mb-1">Resposta do equipamento</div>
            <PluginOutput
              :output="run.output"
              :kind="kind"
              :presentation="presentation"
              class="mb-4"
            />
          </template>
          <v-expansion-panels variant="accordion">
            <v-expansion-panel>
              <v-expansion-panel-title>
                Acessos ao equipamento ({{ run.transcript.length }})
              </v-expansion-panel-title>
              <v-expansion-panel-text>
                <PluginTranscript :entries="run.transcript" />
              </v-expansion-panel-text>
            </v-expansion-panel>
          </v-expansion-panels>
        </template>
      </v-card-text>

      <v-card-actions>
        <v-btn
          v-if="run?.status === 'running'"
          color="error"
          variant="flat"
          prepend-icon="mdi-stop"
          @click="store.cancelRun(run.id)"
        >
          Parar
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" @click="close(false)">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { OutputKind } from '@/bindings/OutputKind'
import { usePluginsStore } from '@/stores/plugins'
import { runStatusPresentation } from '@/utils/pluginPresentation'
import PluginOutput from './PluginOutput.vue'
import type { OutputPresentation } from '@/utils/pluginPresentation'
import PluginTranscript from './PluginTranscript.vue'

const props = defineProps<{
  modelValue: boolean
  deviceId: number
  deviceName: string
  runId: number | null
  title: string
  kind?: OutputKind
  /** Títulos e formatos da saída — a própria ação. */
  presentation?: OutputPresentation
}>()

const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const store = usePluginsStore()
const run = computed(() => store.runById(props.deviceId, props.runId))
const status = computed(() => runStatusPresentation(run.value?.status ?? 'running'))

function close(value = false) {
  emit('update:modelValue', value)
}
</script>

<style scoped>
.response {
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 320px;
  overflow: auto;
}
</style>
