<template>
  <div>
    <PluginRunDialog
      v-if="state.deviceId !== null"
      v-model="state.open"
      :device-id="state.deviceId"
      :device-name="state.deviceName"
      :run-id="state.runId"
      :title="state.title"
    />
    <v-snackbar v-model="failure.show" color="error" timeout="5000">
      {{ failure.text }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
/**
 * Abre a execução de plugin de uma etapa do teste (`runId`) no mesmo diálogo
 * da aba de plugins do equipamento: transcrição a um clique.
 */
import { reactive } from 'vue'
import { usePluginsStore } from '@/stores/plugins'
import PluginRunDialog from '@/components/plugins/PluginRunDialog.vue'

const plugins = usePluginsStore()

const state = reactive({
  open: false,
  runId: null as number | null,
  deviceId: null as number | null,
  deviceName: '',
  title: '',
})
const failure = reactive({ show: false, text: '' })

async function open(runId: number, title: string, deviceName = '') {
  try {
    const run = await plugins.fetchRun(runId)
    Object.assign(state, { runId, deviceId: run.deviceId, deviceName, title, open: true })
  } catch (err: unknown) {
    Object.assign(failure, {
      show: true,
      text: err instanceof Error && err.message ? err.message : 'Execução não encontrada.',
    })
  }
}

defineExpose({ open })
</script>
