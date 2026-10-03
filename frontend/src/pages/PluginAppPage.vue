<template>
  <div>
    <PageHeader :title="title" :subtitle="subtitle">
      <template #actions>
        <v-btn
          v-if="authStore.isAdmin && spec?.statusAction"
          color="primary"
          variant="flat"
          prepend-icon="mdi-refresh"
          :loading="reading"
          :disabled="readyIds.length === 0"
          @click="refresh"
        >
          Atualizar
        </v-btn>
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
      <!-- O resumo, em uma linha -->
      <div class="d-flex flex-wrap align-center ga-2 mb-4">
        <v-chip color="primary" variant="tonal" prepend-icon="mdi-router-network">
          {{ view.members.length }} equipamento(s)
        </v-chip>
        <v-chip
          v-if="matrix"
          color="primary"
          variant="tonal"
          :prepend-icon="matrix.icon ?? 'mdi-format-list-bulleted'"
        >
          {{ rows.length }} {{ (matrix.title ?? 'itens').toLowerCase() }}
        </v-chip>
        <v-chip
          v-if="detailTotal !== null"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-account-multiple"
        >
          {{ detailTotal }} {{ detailLabel.toLowerCase() }}
        </v-chip>
        <span class="text-body-2">
          {{
            reading
              ? 'Lendo os equipamentos…'
              : statusBatch
                ? 'Lido ' + formatRelativeTime(statusBatch.finishedAt)
                : 'Ainda não lido'
          }}
        </span>
      </div>

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
        <v-tab
          v-if="matrix"
          value="items"
          :prepend-icon="matrix.icon ?? 'mdi-format-list-bulleted'"
        >
          {{ matrix.title ?? 'Itens' }}
        </v-tab>
        <v-tab value="members" prepend-icon="mdi-router-network">Equipamentos</v-tab>
        <v-tab v-if="tools.length > 0" value="tools" prepend-icon="mdi-tune-variant">
          Avançado
        </v-tab>
        <v-tab v-if="fleetSchema" value="settings" prepend-icon="mdi-cog-outline">
          Configuração
        </v-tab>
        <v-tab value="runs" prepend-icon="mdi-history">Histórico</v-tab>
        <v-tab value="about" prepend-icon="mdi-book-open-variant">Como usar</v-tab>
      </v-tabs>

      <v-window v-model="tab">
        <v-window-item v-if="matrix" value="items">
          <FleetItems
            :view="view"
            :matrix="matrix"
            :can-write="authStore.isAdmin"
            @add="addItem"
            @edit="editItem"
            @remove="removeItem"
            @refresh="refresh"
          />
        </v-window-item>

        <v-window-item value="members">
          <FleetMembers
            :view="view"
            :can-write="authStore.isAdmin"
            :device-action-title="deviceFleetAction?.title"
            @notify="notify"
            @create-device="deviceDialog = true"
            @configure="configureDevice"
          />
        </v-window-item>

        <v-window-item v-if="tools.length > 0" value="tools">
          <FleetTools :view="view" :can-write="authStore.isAdmin" @run="(id) => openAction(id)" />
        </v-window-item>

        <v-window-item v-if="fleetSchema" value="settings">
          <v-card border flat class="rounded-lg">
            <v-card-text>
              <v-alert type="info" variant="tonal" density="compact" class="mb-4">
                Esta é a configuração desejada, guardada aqui no sistema. Salvar não altera os
                equipamentos; as ações do aplicativo levam a configuração até eles.
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

        <v-window-item value="runs">
          <div v-if="view.batches.length === 0" class="text-body-2">Nada feito ainda.</div>
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

      <FleetActionFlow
        v-model="flow.open"
        :request="flow.request"
        :members="view.members"
        :plugin-id="view.plugin.id"
        :active="view.plugin.status === 'active'"
        :can-write="authStore.isAdmin"
        :spec="spec"
        @changed="refresh"
        @open-run="openTranscript"
      />
    </template>

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
import FleetActionFlow from '@/components/plugins/fleet/FleetActionFlow.vue'
import FleetBatchCard from '@/components/plugins/fleet/FleetBatchCard.vue'
import FleetItems from '@/components/plugins/fleet/FleetItems.vue'
import FleetMembers from '@/components/plugins/fleet/FleetMembers.vue'
import FleetTools from '@/components/plugins/fleet/FleetTools.vue'
import SettingsForm from '@/components/plugins/settings/SettingsForm.vue'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import type { Device } from '@/stores/devices'
import { usePluginAppsStore } from '@/stores/pluginApps'
import {
  itemRows,
  outputsByDevice,
  paramsFromItem,
  toolsOf,
  type FlowRequest,
  type ItemRow,
} from '@/utils/fleetContext'
import { formatDateTime, formatRelativeTime } from '@/utils/formatters'
import {
  batchStatusPresentation,
  devicePrefillFor,
  outputLabel,
  statusPresentation,
  type FollowUp,
} from '@/utils/pluginPresentation'
import { defaultsOf, normalizeValue } from '@/utils/pluginSettings'

const route = useRoute()
const store = usePluginAppsStore()
const authStore = useAuthStore()

const tab = ref('items')
const deviceDialog = ref(false)
const settingsForm = ref<{ validate: () => Promise<boolean> } | null>(null)
const draft = ref<Record<string, unknown>>({})
const saving = ref(false)
const feedback = reactive({ show: false, text: '', color: 'success' })
const flow = reactive({ open: false, request: null as FlowRequest | null })
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
const matrix = computed(() => spec.value?.matrix ?? null)
const title = computed(() => spec.value?.title ?? view.value?.plugin.name ?? 'Aplicativo')
const subtitle = computed(
  () => spec.value?.description ?? view.value?.plugin.description ?? undefined
)
const fleetSchema = computed(() => view.value?.plugin.settings?.fleet ?? null)
const tools = computed(() => toolsOf(spec.value))
const readyIds = computed(() =>
  (view.value?.members ?? [])
    .filter((member) => member.credentialsReady)
    .map((member) => member.deviceId)
)
const deviceFleetAction = computed(() => fleetActionOf(spec.value?.deviceAction ?? ''))

// --- A última leitura dos equipamentos (resumo do topo) ---
const statusBatch = computed(() =>
  spec.value?.statusAction ? store.latestBatch(pluginId.value, spec.value.statusAction) : null
)
const reading = computed(
  () =>
    view.value?.batches.some(
      (batch) => batch.action === spec.value?.statusAction && batch.status === 'running'
    ) ?? false
)
const rows = computed(() =>
  matrix.value
    ? itemRows(outputsByDevice(statusBatch.value), matrix.value, view.value?.members ?? [])
    : []
)
const detailTotal = computed(() => {
  if (!matrix.value?.detail || rows.value.length === 0) return null
  return rows.value.reduce((sum, row) => sum + (row.detailTotal ?? 0), 0)
})
const detailLabel = computed(() => {
  const status = view.value?.plugin.actions.find((action) => action.id === spec.value?.statusAction)
  return outputLabel(matrix.value?.detail ?? '', status?.labels)
})

watch(
  pluginId,
  async (id) => {
    if (!Number.isFinite(id)) return
    await store.loadFleet(id)
    tab.value = matrix.value ? 'items' : 'members'
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
  if (!authStore.isAdmin || reading.value) return
  const finished = statusBatch.value?.finishedAt
  const age = finished ? Date.now() - new Date(finished).getTime() : Infinity
  if (age >= STALE_MS) refresh()
}

/** Lê os equipamentos de novo (só leitura). */
function refresh() {
  const statusAction = spec.value?.statusAction
  if (!statusAction || readyIds.value.length === 0) return
  void store
    .runAction(pluginId.value, statusAction, readyIds.value, false)
    .catch((err: unknown) => notify(describe(err, 'Falha ao ler os equipamentos'), 'error'))
}

// --- Ações: todas abrem o mesmo fluxo (formulário → revisar → aplicar) ---

/** A ação da frota pelo id (a de estado e as pré-visualizações também valem). */
function fleetActionOf(id: string): FleetAction | null {
  const found = spec.value?.actions?.find((item) => item.id === id)
  if (found) return found
  if (spec.value?.statusAction === id) return { id, title: 'Atualizar', action: id }
  const owner = spec.value?.actions?.find((item) => item.preview === id)
  return owner ? { id, title: 'Revisão: ' + owner.title, action: id } : null
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

function openAction(
  id: string,
  options: { title?: string; params?: Record<string, unknown>; devices?: number[] } = {}
) {
  const fleetAction = fleetActionOf(id)
  const action = deviceActionOf(fleetAction)
  if (!fleetAction || !action) return
  if (readyIds.value.length === 0) {
    notify('Nenhum equipamento com acesso cadastrado. Veja a aba Equipamentos.', 'warning')
    return
  }
  flow.request = {
    fleetAction,
    action,
    previewAction: deviceActionOf(fleetActionOf(fleetAction.preview ?? '')),
    title: options.title ?? fleetAction.title,
    initialParams: options.params,
    initialDevices: options.devices,
  }
  flow.open = true
}

const itemName = computed(() => matrix.value?.itemName ?? 'item')

function addItem() {
  if (matrix.value?.add) openAction(matrix.value.add, { title: 'Nova ' + itemName.value })
}

function editItem(row: ItemRow) {
  const target = matrix.value?.edit
  if (!target) return
  openAction(target.action, {
    title: `Editar ${itemName.value} “${row.key}”`,
    params: paramsFromItem(target, row.item),
    devices: row.entries.map((entry) => entry.deviceId),
  })
}

function removeItem(row: ItemRow) {
  const target = matrix.value?.remove
  if (!target) return
  openAction(target.action, {
    title: `Remover ${itemName.value} “${row.key}”`,
    params: paramsFromItem(target, row.item),
    devices: row.entries.map((entry) => entry.deviceId),
  })
}

function configureDevice(deviceId: number) {
  const id = spec.value?.deviceAction
  const member = view.value?.members.find((item) => item.deviceId === deviceId)
  if (!id || !member) return
  openAction(id, {
    title: `${deviceFleetAction.value?.title ?? 'Configurar'} — ${member.name}`,
    devices: [deviceId],
  })
}

// --- Histórico ---

/** Aceitar a sugestão de um lote antigo: a ação dela, com o de cada equipamento. */
async function applyNext(_batch: PluginBatchView, followUp: FollowUp) {
  const action = deviceActionOf(fleetActionOf(followUp.action))
  if (!action) return
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
  try {
    await store.runAction(
      pluginId.value,
      followUp.action,
      Object.keys(followUp.devices).map(Number),
      write,
      {},
      followUp.devices
    )
    notify('Aplicando… o andamento aparece aqui no histórico.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao iniciar'), 'error')
  }
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

// --- Equipamentos e configuração guardada ---

/**
 * O equipamento cadastrado daqui já entra no aplicativo. Sem o acesso
 * (SSH/HTTP) ele ainda não responde: o cartão dele mostra onde cadastrar.
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
    notify('Configuração salva.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao salvar'), 'error')
  } finally {
    saving.value = false
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
</script>
