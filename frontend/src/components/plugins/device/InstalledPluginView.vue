<template>
  <div>
    <!-- Cabeçalho do plugin -->
    <div class="d-flex align-center flex-wrap ga-2 mb-3">
      <v-icon color="primary" size="28">{{ item.plugin.list?.icon ?? 'mdi-puzzle' }}</v-icon>
      <div class="min-w-0">
        <div class="text-subtitle-1 font-weight-bold">
          {{ item.plugin.list?.title ?? item.plugin.name }}
          <span class="text-body-small">v{{ item.plugin.version }}</span>
        </div>
        <div v-if="item.plugin.description" class="text-body-2">
          {{ item.plugin.description }}
        </div>
      </div>
      <v-spacer></v-spacer>
      <v-chip size="small" :color="status.color" variant="flat">{{ status.label }}</v-chip>
      <v-chip size="small" :color="compat.color" variant="tonal">
        <v-icon start size="14">{{ compat.icon }}</v-icon>
        {{ compat.label }}
      </v-chip>
    </div>

    <v-alert v-if="blocked" type="error" variant="tonal" density="compact" class="mb-3">
      {{
        item.plugin.status === 'quarantine'
          ? 'Plugin em quarentena: aceite a revisão de segurança antes de usar.'
          : 'Plugin desativado na biblioteca.'
      }}
    </v-alert>
    <v-alert
      v-else-if="item.plugin.status !== 'active'"
      type="warning"
      variant="tonal"
      density="compact"
      class="mb-3"
    >
      Plugin ainda não ativo: cada acesso ao equipamento vai pedir sua aprovação.
      <template v-if="item.plugin.status === 'tested'" #append>
        <v-btn size="small" color="success" variant="flat" @click="promote">Ativar</v-btn>
      </template>
    </v-alert>

    <!-- Tela própria do plugin, ou a lista de ações -->
    <DeviceItemList
      v-if="item.plugin.list"
      :device-id="deviceId"
      :plugin-id="item.plugin.id"
      :list="item.plugin.list"
      :actions="item.plugin.actions"
      :autoload="item.plugin.status === 'active'"
      :disabled="blocked"
      :last-action-run-id="runDialog.runId"
      @run="openAction"
      @error="(message) => notify(message, 'error')"
    />
    <v-list v-else density="compact" border class="rounded-lg">
      <v-list-item
        v-for="action in item.plugin.actions"
        :key="action.id"
        :title="action.title"
        :subtitle="action.description ?? action.id"
      >
        <template #prepend>
          <v-icon :color="effectPresentation(action.effect).color">
            {{ effectPresentation(action.effect).icon }}
          </v-icon>
        </template>
        <template #append>
          <v-btn
            size="small"
            :color="action.effect === 'write' ? 'error' : 'primary'"
            variant="flat"
            prepend-icon="mdi-play"
            :disabled="blocked"
            @click="openAction(action, {})"
          >
            Executar
          </v-btn>
        </template>
      </v-list-item>
    </v-list>

    <!-- Ajuste deste equipamento (configuração `device` do plugin) -->
    <v-card v-if="deviceSchema" border flat class="rounded-lg mt-4">
      <v-card-title class="d-flex align-center ga-2 text-subtitle-1 font-weight-bold">
        <v-icon color="primary">mdi-tune-variant</v-icon>
        Ajustes deste equipamento
      </v-card-title>
      <v-card-subtitle>
        Guardados no sistema; valem só aqui e sobrepõem a configuração geral do aplicativo.
      </v-card-subtitle>
      <v-card-text>
        <SettingsForm ref="settingsForm" v-model="settingsDraft" :schema="deviceSchema" />
      </v-card-text>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-content-save-outline"
          :loading="savingSettings"
          @click="saveSettings"
        >
          Salvar ajustes
        </v-btn>
      </v-card-actions>
    </v-card>

    <!-- Operação do plugin neste equipamento -->
    <div class="d-flex flex-wrap ga-2 mt-4">
      <v-btn
        v-if="item.plugin.surfaces.includes('fleet')"
        size="small"
        color="secondary"
        variant="flat"
        prepend-icon="mdi-apps"
        :to="'/apps/' + item.plugin.id"
      >
        Abrir aplicativo
      </v-btn>
      <v-btn
        size="small"
        color="success"
        variant="flat"
        prepend-icon="mdi-check-decagram-outline"
        :disabled="blocked"
        @click="validate"
      >
        Validar neste equipamento
      </v-btn>
      <v-btn
        size="small"
        color="primary"
        variant="flat"
        prepend-icon="mdi-code-braces"
        @click="editorOpen = true"
      >
        {{ item.plugin.source === 'builtin' ? 'Ver código' : 'Editar' }}
      </v-btn>
      <v-btn
        size="small"
        color="secondary"
        variant="flat"
        prepend-icon="mdi-download-outline"
        @click="exportPlugin"
      >
        Exportar
      </v-btn>
      <v-spacer></v-spacer>
      <v-btn
        size="small"
        color="error"
        variant="outlined"
        prepend-icon="mdi-puzzle-remove-outline"
        @click="uninstall"
      >
        Desinstalar
      </v-btn>
    </div>

    <v-expansion-panels class="mt-4" variant="accordion">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <v-icon color="primary" class="mr-2">mdi-book-open-variant</v-icon>
          Como usar, compatibilidade e testes
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <PluginAbout
            :plugin-id="item.plugin.id"
            :device-id="deviceId"
            :last-test-ok="item.plugin.lastTestOk"
            @error="(message) => notify(message, 'error')"
          />
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>

    <PluginActionDialog
      v-model="actionDialog.open"
      :action="actionDialog.action"
      :initial-params="actionDialog.params"
      :plugin-name="item.plugin.name"
      :device-name="deviceName"
      :active="item.plugin.status === 'active'"
      @run="runAction"
    />
    <PluginEditorDialog
      v-model="editorOpen"
      :plugin-id="item.plugin.id"
      :device-id="deviceId"
      @saved="store.loadDevice(deviceId)"
    />
    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="6000" location="bottom">
      {{ feedback.text }}
    </v-snackbar>
    <PluginRunDialog
      v-model="runDialog.open"
      :device-id="deviceId"
      :device-name="deviceName"
      :run-id="runDialog.runId"
      :title="runDialog.title"
      :kind="runDialog.kind"
      :presentation="runDialog.presentation"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import type { OutputKind } from '@/bindings/OutputKind'
import type { PluginAction } from '@/bindings/PluginAction'
import { usePluginsStore } from '@/stores/plugins'
import { confirm } from '@/composables/useConfirm'
import {
  compatPresentation,
  effectPresentation,
  statusPresentation,
} from '@/utils/pluginPresentation'
import PluginActionDialog from '@/components/plugins/PluginActionDialog.vue'
import PluginRunDialog from '@/components/plugins/PluginRunDialog.vue'
import PluginEditorDialog from '@/components/plugins/PluginEditorDialog.vue'
import SettingsForm from '@/components/plugins/settings/SettingsForm.vue'
import { defaultsOf, normalizeValue } from '@/utils/pluginSettings'
import PluginAbout from './PluginAbout.vue'
import DeviceItemList from './DeviceItemList.vue'

const props = defineProps<{
  item: DevicePluginItem
  deviceId: number
  deviceName: string
}>()

const emit = defineEmits<{
  uninstalled: []
}>()

const store = usePluginsStore()
const editorOpen = ref(false)
const feedback = reactive({ show: false, text: '', color: 'success' })

function notify(text: string, color = 'success') {
  feedback.text = text
  feedback.color = color
  feedback.show = true
}
const actionDialog = reactive({
  open: false,
  action: null as PluginAction | null,
  params: {} as Record<string, unknown>,
})
const runDialog = reactive({
  open: false,
  runId: null as number | null,
  title: '',
  kind: undefined as OutputKind | undefined,
  presentation: undefined as PluginAction | undefined,
})

const settingsForm = ref<{ validate: () => Promise<boolean> } | null>(null)
const settingsDraft = ref<Record<string, unknown>>({})
const savingSettings = ref(false)
const deviceSchema = computed(() => props.item.plugin.settings?.device ?? null)

watch(
  () => props.item.deviceSettings,
  (saved) => {
    settingsDraft.value = { ...defaultsOf(deviceSchema.value), ...(saved ?? {}) }
  },
  { immediate: true }
)

async function saveSettings() {
  if (settingsForm.value && !(await settingsForm.value.validate())) return
  savingSettings.value = true
  try {
    await store.saveDeviceSettings(
      props.deviceId,
      props.item.plugin.id,
      normalizeValue(deviceSchema.value, settingsDraft.value)
    )
    notify('Ajustes salvos. Aplique a configuração para levar ao equipamento.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao salvar os ajustes'), 'error')
  } finally {
    savingSettings.value = false
  }
}

const status = computed(() => statusPresentation(props.item.plugin.status))
const compat = computed(() => compatPresentation(props.item.compat))
const blocked = computed(
  () => props.item.plugin.status === 'quarantine' || props.item.plugin.status === 'disabled'
)

function describe(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

/** O formulário tem algo a perguntar: algum parâmetro ainda sem valor. */
function needsForm(action: PluginAction, params: Record<string, unknown>): boolean {
  const properties = action.params?.properties
  if (typeof properties !== 'object' || properties === null) return false
  return Object.keys(properties).some((name) => params[name] === undefined)
}

/**
 * Leitura sem nada a perguntar roda direto e abre o resultado; o que altera
 * o equipamento, ou pede parâmetro, passa pelo diálogo.
 */
function openAction(action: PluginAction, params: Record<string, unknown>) {
  if (action.effect === 'read' && !needsForm(action, params)) {
    actionDialog.action = action
    void runAction(params, false)
    return
  }
  actionDialog.action = action
  actionDialog.params = params
  actionDialog.open = true
}

async function runAction(params: Record<string, unknown>, confirmWrite: boolean) {
  const action = actionDialog.action
  if (!action) return
  runDialog.runId = null
  runDialog.title = action.title
  runDialog.kind = action.output
  runDialog.presentation = action
  runDialog.open = true
  try {
    runDialog.runId = await store.runAction(
      props.deviceId,
      props.item.plugin.id,
      action.id,
      params,
      confirmWrite
    )
  } catch (err: unknown) {
    runDialog.open = false
    notify(describe(err, 'Falha ao iniciar a execução'), 'error')
  }
}

async function validate() {
  runDialog.runId = null
  runDialog.title = 'Validação no equipamento'
  runDialog.kind = 'json'
  runDialog.open = true
  try {
    runDialog.runId = await store.validatePlugin(props.deviceId, props.item.plugin.id)
  } catch (err: unknown) {
    runDialog.open = false
    notify(describe(err, 'Falha ao iniciar a validação'), 'error')
  }
}

async function promote() {
  const ok = await confirm({
    title: 'Ativar plugin',
    message: `Ativar “${props.item.plugin.name}”? Plugin ativo roda sem pedir aprovação a cada acesso (ações que alteram o equipamento continuam pedindo confirmação).`,
    confirmText: 'Ativar',
    confirmColor: 'success',
  })
  if (!ok) return
  try {
    await store.lifecycle(props.item.plugin.id, 'promote', props.deviceId)
    notify('Plugin ativado.', 'success')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao ativar'), 'error')
  }
}

async function exportPlugin() {
  try {
    await store.exportPlugin(props.item.plugin)
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao exportar'), 'error')
  }
}

async function uninstall() {
  const ok = await confirm({
    title: 'Desinstalar plugin',
    message: `Tirar “${props.item.plugin.name}” deste equipamento? Nada é alterado no equipamento; só a aba sai. O histórico de execuções fica.`,
    confirmText: 'Desinstalar',
    confirmColor: 'error',
  })
  if (!ok) return
  try {
    await store.uninstallPlugin(props.deviceId, props.item.plugin.id)
    emit('uninstalled')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao desinstalar'), 'error')
  }
}
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
</style>
