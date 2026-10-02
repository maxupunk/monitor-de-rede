<template>
  <div>
    <PageHeader :title="title" :subtitle="subtitle">
      <template #actions>
        <v-btn
          v-if="authStore.isAdmin"
          color="secondary"
          variant="flat"
          prepend-icon="mdi-puzzle-outline"
          to="/plugins"
        >
          Plugins
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="store.error" type="error" variant="tonal" class="mb-4">{{
      store.error
    }}</v-alert>
    <v-progress-linear v-if="!view && store.loading" indeterminate color="primary" />

    <template v-if="view">
      <v-alert
        v-if="view.plugin.status !== 'active'"
        type="warning"
        variant="tonal"
        density="compact"
        class="mb-4"
      >
        Plugin {{ statusPresentation(view.plugin.status).label.toLowerCase() }}: cada acesso a cada
        equipamento vai pedir sua aprovação até ele ser testado e ativado.
      </v-alert>

      <v-tabs v-model="tab" color="primary" class="mb-4" show-arrows>
        <v-tab value="overview" prepend-icon="mdi-view-dashboard-outline">Visão geral</v-tab>
        <v-tab v-if="fleetSchema" value="settings" prepend-icon="mdi-cog-outline">
          Configuração
        </v-tab>
        <v-tab value="members" prepend-icon="mdi-router-network">
          Equipamentos
          <v-chip size="x-small" color="primary" variant="tonal" class="ml-2">
            {{ view.members.length }}
          </v-chip>
        </v-tab>
        <v-tab value="runs" prepend-icon="mdi-history">Execuções</v-tab>
        <v-tab value="about" prepend-icon="mdi-book-open-variant">Como usar</v-tab>
      </v-tabs>

      <v-window v-model="tab">
        <v-window-item value="overview">
          <FleetOverview
            :view="view"
            :can-write="authStore.isAdmin"
            @run="openRun"
            @create-device="deviceDialog = true"
          />
        </v-window-item>

        <v-window-item v-if="fleetSchema" value="settings">
          <v-card border flat class="rounded-lg">
            <v-card-text>
              <v-alert type="info" variant="tonal" density="compact" class="mb-4">
                Esta é a configuração desejada da rede, guardada aqui no sistema. Salvar não altera
                os equipamentos: use <strong>Pré-visualizar</strong> e depois
                <strong>Aplicar</strong> na Visão geral.
              </v-alert>
              <SettingsForm
                ref="settingsForm"
                v-model="draft"
                :schema="fleetSchema"
                :disabled="!authStore.isAdmin"
              />
            </v-card-text>
            <v-card-actions v-if="authStore.isAdmin">
              <v-spacer></v-spacer>
              <v-btn variant="text" color="secondary" @click="resetDraft">Descartar</v-btn>
              <v-btn
                color="primary"
                variant="flat"
                prepend-icon="mdi-content-save-outline"
                :loading="saving"
                @click="saveSettings"
              >
                Salvar configuração
              </v-btn>
            </v-card-actions>
          </v-card>
        </v-window-item>

        <v-window-item value="members">
          <FleetMembers
            :view="view"
            :can-write="authStore.isAdmin"
            @notify="notify"
            @create-device="deviceDialog = true"
          />
        </v-window-item>

        <v-window-item value="runs">
          <div v-if="view.batches.length === 0" class="text-body-2">Nenhuma execução ainda.</div>
          <v-expansion-panels v-else variant="accordion">
            <v-expansion-panel v-for="batch in view.batches" :key="batch.id">
              <v-expansion-panel-title>
                <div class="d-flex align-center flex-wrap ga-2">
                  <v-icon :color="batchStatusPresentation(batch.status).color" size="18">
                    {{ batchStatusPresentation(batch.status).icon }}
                  </v-icon>
                  <span class="font-weight-bold">{{ batch.title }}</span>
                  <span class="text-body-2">{{ formatDateTime(batch.createdAt) }}</span>
                  <v-chip v-if="batch.hasPatch" size="x-small" color="success" variant="flat">
                    Sugestão pendente
                  </v-chip>
                </div>
              </v-expansion-panel-title>
              <v-expansion-panel-text>
                <FleetBatchCard
                  :batch="batch"
                  :kind="kindOf(batch.action)"
                  :labels="labelsOf(batch.action)"
                  :result-labels="fleetActionOf(batch.action)?.labels"
                  :can-write="authStore.isAdmin"
                  @cancel="cancel"
                  @apply-patch="applyPatch"
                  @apply-next="applyNext"
                  @open-run="openTranscript"
                />
              </v-expansion-panel-text>
            </v-expansion-panel>
          </v-expansion-panels>
        </v-window-item>

        <v-window-item value="about">
          <PluginAbout
            :plugin-id="view.plugin.id"
            :last-test-ok="view.plugin.lastTestOk"
            @error="(text) => notify(text, 'error')"
          />
        </v-window-item>
      </v-window>
    </template>

    <FleetRunDialog
      v-if="view"
      v-model="runDialog.open"
      :fleet-action="runDialog.fleetAction"
      :action="runDialog.action"
      :preview-action="deviceActionOf(fleetActionOf(runDialog.fleetAction?.preview ?? ''))"
      :initial-params="runDialog.params"
      :initial-devices="runDialog.devices"
      :members="view.members"
      :plugin-id="view.plugin.id"
      :plugin-name="view.plugin.name"
      :active="view.plugin.status === 'active'"
      :spec="spec"
      @run="start"
    />

    <v-dialog v-model="live.open" max-width="820" scrollable>
      <v-card class="rounded-lg">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-play-network-outline</v-icon>
          {{ live.title }}
        </v-card-title>
        <v-card-text>
          <FleetBatchCard
            v-if="liveBatch"
            :batch="liveBatch"
            :kind="kindOf(liveBatch.action)"
            :labels="labelsOf(liveBatch.action)"
            :result-labels="fleetActionOf(liveBatch.action)?.labels"
            :can-write="authStore.isAdmin"
            @cancel="cancel"
            @apply-patch="applyPatch"
            @apply-next="applyNext"
            @open-run="openTranscript"
          />
          <div v-else class="d-flex align-center ga-3 py-4">
            <v-progress-circular indeterminate color="primary" size="24" />
            Iniciando nos equipamentos…
          </div>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn color="primary" variant="flat" @click="live.open = false">Fechar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <PluginRunDialog
      v-model="transcript.open"
      :device-id="transcript.deviceId"
      :device-name="transcript.deviceName"
      :run-id="transcript.runId"
      :title="transcript.title"
      :kind="transcript.kind"
      :labels="transcript.labels"
    />

    <DeviceDialog
      v-if="view"
      v-model="deviceDialog"
      :prefill-data="devicePrefillFor(view.plugin.matcher)"
      @saved="onDeviceCreated"
    />

    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="6000" location="bottom">
      {{ feedback.text }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import type { BatchDevice } from '@/bindings/BatchDevice'
import type { FleetAction } from '@/bindings/FleetAction'
import type { OutputKind } from '@/bindings/OutputKind'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import PageHeader from '@/components/PageHeader.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'
import PluginRunDialog from '@/components/plugins/PluginRunDialog.vue'
import PluginAbout from '@/components/plugins/device/PluginAbout.vue'
import FleetBatchCard from '@/components/plugins/fleet/FleetBatchCard.vue'
import FleetMembers from '@/components/plugins/fleet/FleetMembers.vue'
import FleetOverview from '@/components/plugins/fleet/FleetOverview.vue'
import FleetRunDialog from '@/components/plugins/fleet/FleetRunDialog.vue'
import SettingsForm from '@/components/plugins/settings/SettingsForm.vue'
import { useAuthStore } from '@/stores/auth'
import type { Device } from '@/stores/devices'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { formatDateTime } from '@/utils/formatters'
import {
  batchStatusPresentation,
  devicePrefillFor,
  statusPresentation,
  type FollowUp,
} from '@/utils/pluginPresentation'
import { confirm } from '@/composables/useConfirm'
import { defaultsOf, fieldsOf, normalizeValue } from '@/utils/pluginSettings'

const route = useRoute()
const store = usePluginAppsStore()
const authStore = useAuthStore()

const tab = ref('overview')
const deviceDialog = ref(false)
const settingsForm = ref<{ validate: () => Promise<boolean> } | null>(null)
const draft = ref<Record<string, unknown>>({})
const saving = ref(false)
const feedback = reactive({ show: false, text: '', color: 'success' })
const runDialog = reactive({
  open: false,
  fleetAction: null as FleetAction | null,
  action: null as PluginAction | null,
  params: {} as Record<string, unknown>,
  devices: undefined as number[] | undefined,
})
const live = reactive({ open: false, title: '', batchId: null as number | null })
const transcript = reactive({
  open: false,
  deviceId: 0,
  deviceName: '',
  runId: null as number | null,
  title: '',
  kind: undefined as OutputKind | undefined,
  labels: undefined as Record<string, string> | undefined,
})

const pluginId = computed(() => Number(route.params.id))
const view = computed(() => store.fleets[pluginId.value] ?? null)
const spec = computed(() => view.value?.plugin.fleet ?? null)
const title = computed(() => spec.value?.title ?? view.value?.plugin.name ?? 'Aplicativo')
const subtitle = computed(
  () => spec.value?.description ?? view.value?.plugin.description ?? undefined
)
const fleetSchema = computed(() => view.value?.plugin.settings?.fleet ?? null)
const liveBatch = computed(() => store.batchById(pluginId.value, live.batchId))

watch(
  pluginId,
  async (id) => {
    if (!Number.isFinite(id)) return
    await store.loadFleet(id)
    resetDraft()
    readIfStale()
  },
  { immediate: true }
)

/** Leitura mais velha que isto é refeita ao abrir a página. */
const STALE_MS = 10 * 60 * 1000

/**
 * Abrir o aplicativo mostra os equipamentos como estão: sem leitura recente,
 * a ação de estado roda uma vez (só leitura; o resultado chega pelo SSE).
 */
function readIfStale() {
  const statusAction = spec.value?.statusAction
  const ready = (view.value?.members ?? []).filter((member) => member.credentialsReady)
  if (!statusAction || ready.length === 0 || !authStore.isAdmin) return
  const last = store.latestBatch(pluginId.value, statusAction)
  const running = view.value?.batches.some(
    (batch) => batch.action === statusAction && batch.status === 'running'
  )
  const age = last?.finishedAt ? Date.now() - new Date(last.finishedAt).getTime() : Infinity
  if (running || age < STALE_MS) return
  void store
    .runAction(
      pluginId.value,
      statusAction,
      ready.map((member) => member.deviceId),
      false
    )
    .catch(() => undefined)
}

/**
 * O equipamento cadastrado daqui já entra no aplicativo. Sem o acesso
 * (SSH/HTTP) ele ainda não responde: a aba Equipamentos mostra onde cadastrar.
 */
async function onDeviceCreated(device: Device) {
  try {
    await store.addMember(pluginId.value, device.id)
    tab.value = 'members'
    notify(`${device.name} entrou em ${title.value}. Cadastre o acesso dele para usar.`)
  } catch (err: unknown) {
    notify(describe(err, 'Equipamento cadastrado, mas não entrou no aplicativo'), 'error')
  }
}

function notify(text: string, color = 'success') {
  feedback.text = text
  feedback.color = color
  feedback.show = true
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

function resetDraft() {
  draft.value = { ...defaultsOf(fleetSchema.value), ...(view.value?.settings ?? {}) }
}

async function saveSettings() {
  if (!fleetSchema.value) return
  if (settingsForm.value && !(await settingsForm.value.validate())) return
  saving.value = true
  try {
    await store.saveFleetSettings(pluginId.value, normalizeValue(fleetSchema.value, draft.value))
    resetDraft()
    notify('Configuração salva. Pré-visualize e aplique para levar aos equipamentos.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao salvar'), 'error')
  } finally {
    saving.value = false
  }
}

/** A ação da frota pelo id (a de estado e as pré-visualizações também valem). */
function fleetActionOf(id: string): FleetAction | null {
  const found = spec.value?.actions?.find((item) => item.id === id)
  if (found) return found
  if (spec.value?.statusAction === id) return { id, title: 'Atualizar estado', action: id }
  const owner = spec.value?.actions?.find((item) => item.preview === id)
  return owner ? { id, title: 'Pré-visualizar: ' + owner.title, action: id } : null
}

function deviceActionOf(fleetAction: FleetAction | null): PluginAction | null {
  if (!fleetAction) return null
  return view.value?.plugin.actions.find((action) => action.id === fleetAction.action) ?? null
}

function kindOf(id: string): OutputKind | undefined {
  return deviceActionOf(fleetActionOf(id))?.output
}

function labelsOf(id: string): Record<string, string> | undefined {
  return deviceActionOf(fleetActionOf(id))?.labels
}

/**
 * Leitura sem parâmetro roda direto em todos os equipamentos prontos; o que
 * altera ou pergunta algo passa pelo diálogo (escolha de equipamentos e ciência).
 * Vindo de uma coluna da grade, o diálogo abre preenchido.
 */
function openRun(id: string, params: Record<string, unknown> = {}, devices?: number[]) {
  const fleetAction = fleetActionOf(id)
  const action = deviceActionOf(fleetAction)
  if (!fleetAction || !action) return
  if (action.effect === 'read' && fieldsOf(action.params).length === 0) {
    const ready = (view.value?.members ?? []).filter((member) => member.credentialsReady)
    if (ready.length === 0) {
      notify('Nenhum equipamento com acesso cadastrado.', 'warning')
      return
    }
    void launch(
      fleetAction,
      ready.map((member) => member.deviceId),
      {},
      false
    )
    return
  }
  runDialog.fleetAction = fleetAction
  runDialog.action = action
  runDialog.params = params
  runDialog.devices = devices
  runDialog.open = true
}

function start(deviceIds: number[], params: Record<string, unknown>, confirmWrite: boolean) {
  if (runDialog.fleetAction) void launch(runDialog.fleetAction, deviceIds, params, confirmWrite)
}

async function launch(
  fleetAction: FleetAction,
  deviceIds: number[],
  params: Record<string, unknown>,
  confirmWrite: boolean,
  deviceParams?: Record<number, Record<string, unknown>>
) {
  live.title = fleetAction.title
  live.batchId = null
  live.open = fleetAction.id !== spec.value?.statusAction
  try {
    live.batchId = await store.runAction(
      pluginId.value,
      fleetAction.id,
      deviceIds,
      confirmWrite,
      params,
      deviceParams
    )
  } catch (err: unknown) {
    live.open = false
    notify(describe(err, 'Falha ao iniciar'), 'error')
  }
}

/** Aceitar a sugestão de um consolidado: a ação dela, com o de cada equipamento. */
async function applyNext(_batch: PluginBatchView, followUp: FollowUp) {
  const fleetAction = fleetActionOf(followUp.action)
  const action = deviceActionOf(fleetAction)
  if (!fleetAction || !action) return
  const count = Object.keys(followUp.devices).length
  const write = action.effect === 'write'
  const ok = await confirm({
    title: followUp.title,
    message: write
      ? `Aplicar em ${count} equipamento(s)? Isso altera a configuração deles; cada um guarda uma cópia e volta atrás se a conferência falhar.`
      : `Rodar em ${count} equipamento(s)?`,
    confirmText: 'Aplicar',
    confirmColor: write ? 'error' : 'primary',
  })
  if (!ok) return
  await launch(fleetAction, Object.keys(followUp.devices).map(Number), {}, write, followUp.devices)
}

async function cancel(batch: PluginBatchView) {
  try {
    await store.cancelBatch(batch.id)
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao cancelar'), 'error')
  }
}

async function applyPatch(batch: PluginBatchView) {
  try {
    await store.applyPatch(batch)
    resetDraft()
    notify('Sugestão aceita: os ajustes entraram na configuração de cada equipamento.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao aceitar a sugestão'), 'error')
  }
}

function openTranscript(device: BatchDevice) {
  const batch = view.value?.batches.find((item) =>
    item.devices.some((entry) => entry.runId === device.runId)
  )
  transcript.deviceId = device.deviceId
  transcript.deviceName = device.deviceName
  transcript.runId = device.runId
  transcript.title = batch?.title ?? 'Execução'
  transcript.kind = batch ? kindOf(batch.action) : undefined
  transcript.labels = batch ? labelsOf(batch.action) : undefined
  transcript.open = true
}
</script>
