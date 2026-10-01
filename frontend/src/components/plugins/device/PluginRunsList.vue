<template>
  <div>
    <div v-if="runs.length === 0" class="text-body-2">Nenhuma execução ainda.</div>
    <v-expansion-panels v-else v-model="openRun" variant="accordion">
      <v-expansion-panel v-for="run in runs" :key="run.id" :value="run.id">
        <v-expansion-panel-title>
          <div class="d-flex align-center flex-wrap ga-2">
            <v-chip size="x-small" :color="runStatusPresentation(run.status).color" variant="flat">
              <v-progress-circular
                v-if="run.status === 'running'"
                indeterminate
                size="10"
                width="2"
                class="mr-1"
              />
              <v-icon v-else start size="12">{{ runStatusPresentation(run.status).icon }}</v-icon>
              {{ runStatusPresentation(run.status).label }}
            </v-chip>
            <span class="font-weight-medium">{{ run.pluginName ?? 'Acesso avulso' }}</span>
            <span class="text-body-small"
              >{{ actionTitle(run) }} · {{ originLabel(run.origin) }}</span
            >
            <span class="text-body-small">{{ formatRelativeTime(run.createdAt) }}</span>
          </div>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <div v-if="run.status === 'running'" class="mb-3">
            <v-btn
              size="small"
              color="error"
              variant="flat"
              prepend-icon="mdi-stop"
              @click="store.cancelRun(run.id)"
            >
              Parar execução
            </v-btn>
          </div>
          <v-alert v-if="run.error" type="error" variant="tonal" density="compact" class="mb-3">
            <pre class="error-text font-mono text-body-small">{{ run.error }}</pre>
          </v-alert>
          <PluginOutput
            v-if="run.output !== null && run.output !== undefined"
            :output="run.output"
            :kind="outputKind(run)"
            class="mb-3"
          />
          <div class="text-subtitle-2 font-weight-bold mb-1">Acessos ao equipamento</div>
          <PluginTranscript :entries="run.transcript" />
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import type { OutputKind } from '@/bindings/OutputKind'
import type { PluginRunView } from '@/bindings/PluginRunView'
import { usePluginsStore } from '@/stores/plugins'
import { formatRelativeTime } from '@/utils/formatters'
import { originLabel, runStatusPresentation } from '@/utils/pluginPresentation'
import PluginOutput from '@/components/plugins/PluginOutput.vue'
import PluginTranscript from '@/components/plugins/PluginTranscript.vue'

const props = defineProps<{
  runs: PluginRunView[]
  plugins: DevicePluginItem[]
}>()

const store = usePluginsStore()
const openRun = ref<number | null>(null)

function actionOf(run: PluginRunView) {
  const item = props.plugins.find((plugin) => plugin.plugin.id === run.pluginId)
  return item?.plugin.actions.find((action) => action.id === run.action)
}

function actionTitle(run: PluginRunView): string {
  return actionOf(run)?.title ?? run.action
}

function outputKind(run: PluginRunView): OutputKind | undefined {
  return actionOf(run)?.output
}
</script>

<style scoped>
.error-text {
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
