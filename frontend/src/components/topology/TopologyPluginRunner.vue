<template>
  <div>
    <PluginActionDialog
      v-model="actionDialog.open"
      :action="actionDialog.action"
      :plugin-name="actionDialog.pluginName"
      :device-name="actionDialog.deviceName"
      :active="actionDialog.active"
      @run="run"
    />
    <PluginRunDialog
      v-if="runDialog.deviceId !== null"
      v-model="runDialog.open"
      :device-id="runDialog.deviceId"
      :device-name="actionDialog.deviceName"
      :run-id="runDialog.runId"
      :title="runDialog.title"
      :kind="runDialog.kind"
      :presentation="runDialog.presentation"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Executa uma ação de plugin a partir do mapa com os mesmos diálogos da aba
 * de plugins do dispositivo (formulário/confirmação de escrita e saída ao
 * vivo pelo SSE). Leitura sem parâmetro roda direto.
 */
import { reactive } from 'vue'
import type { OutputKind } from '@/bindings/OutputKind'
import type { PluginAction } from '@/bindings/PluginAction'
import { usePluginsStore } from '@/stores/plugins'
import type { TopologyNode, TopologyPluginAction } from '@/stores/topology'
import PluginActionDialog from '@/components/plugins/PluginActionDialog.vue'
import PluginRunDialog from '@/components/plugins/PluginRunDialog.vue'

const emit = defineEmits<{ error: [message: string] }>()

const store = usePluginsStore()

const actionDialog = reactive({
  open: false,
  action: null as PluginAction | null,
  pluginId: 0,
  pluginName: '',
  deviceName: '',
  active: true,
})
const runDialog = reactive({
  open: false,
  deviceId: null as number | null,
  runId: null as number | null,
  title: '',
  kind: undefined as OutputKind | undefined,
  presentation: undefined as PluginAction | undefined,
})

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

async function open(node: TopologyNode, item: TopologyPluginAction) {
  // A aba do dispositivo é carregada sob demanda: dá a definição completa da
  // ação e é onde o SSE pendura as execuções que o diálogo acompanha.
  if (!store.deviceViews[node.id]) await store.loadDevice(node.id)
  const entry = store.deviceViews[node.id]?.plugins.find((p) => p.plugin.id === item.pluginId)
  const action = entry?.plugin.actions.find((a) => a.id === item.action) ?? null
  if (!entry || !action) {
    emit('error', store.error ?? 'Esta ação não está mais disponível neste dispositivo.')
    return
  }
  actionDialog.action = action
  actionDialog.pluginId = item.pluginId
  actionDialog.pluginName = entry.plugin.name
  actionDialog.deviceName = node.name
  actionDialog.active = entry.plugin.status === 'active'
  runDialog.deviceId = node.id

  const needsForm = item.hasParams || action.effect !== 'read'
  if (needsForm) {
    actionDialog.open = true
    return
  }
  await run({}, false)
}

async function run(params: Record<string, unknown>, confirmWrite: boolean) {
  const action = actionDialog.action
  const deviceId = runDialog.deviceId
  if (!action || deviceId === null) return
  runDialog.runId = null
  runDialog.title = action.title
  runDialog.kind = action.output
  runDialog.presentation = action
  runDialog.open = true
  try {
    runDialog.runId = await store.runAction(
      deviceId,
      actionDialog.pluginId,
      action.id,
      params,
      confirmWrite
    )
  } catch (err: unknown) {
    runDialog.open = false
    emit('error', describe(err, 'Falha ao iniciar a execução'))
  }
}

defineExpose({ open })
</script>
