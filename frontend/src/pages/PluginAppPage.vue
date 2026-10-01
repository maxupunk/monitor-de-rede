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
          <FleetOverview :view="view" :can-write="authStore.isAdmin" @run="openRun" />
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
          <FleetMembers :view="view" :can-write="authStore.isAdmin" @notify="notify" />
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
      :members="view.members"
      :plugin-name="view.plugin.name"
      :active="view.plugin.status === 'active'"
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
import PluginRunDialog from '@/components/plugins/PluginRunDialog.vue'
import PluginAbout from '@/components/plugins/device/PluginAbout.vue'
import FleetBatchCard from '@/components/plugins/fleet/FleetBatchCard.vue'
import FleetMembers from '@/components/plugins/fleet/FleetMembers.vue'
import FleetOverview from '@/components/plugins/fleet/FleetOverview.vue'
import FleetRunDialog from '@/components/plugins/fleet/FleetRunDialog.vue'
import SettingsForm from '@/components/plugins/settings/SettingsForm.vue'
import { useAuthStore } from '@/stores/auth'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { formatDateTime } from '@/utils/formatters'
import { batchStatusPresentation, statusPresentation } from '@/utils/pluginPresentation'
import { defaultsOf, fieldsOf, normalizeValue } from '@/utils/pluginSettings'

const route = useRoute()
const store = usePluginAppsStore()
const authStore = useAuthStore()

const tab = ref('overview')
const settingsForm = ref<{ validate: () => Promise<boolean> } | null>(null)
const draft = ref<Record<string, unknown>>({})
const saving = ref(false)
const feedback = reactive({ show: false, text: '', color: 'success' })
const runDialog = reactive({
  open: false,
  fleetAction: null as FleetAction | null,
  action: null as PluginAction | null,
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
  },
  { immediate: true }
)

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

/** A ação da frota pelo id (a de estado também vale). */
function fleetActionOf(id: string): FleetAction | null {
  const found = spec.value?.actions?.find((item) => item.id === id)
  if (found) return found
  return spec.value?.statusAction === id ? { id, title: 'Atualizar estado', action: id } : null
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
 */
function openRun(id: string) {
  const fleetAction = fleetActionOf(id)
  const action = deviceActionOf(fleetAction)
  if (!fleetAction || !action) return
  if (action.effect === 'read' && fieldsOf(action.params).length === 0) {
    const ready = (view.value?.members ?? []).filter((member) => member.credentialsReady)
    if (ready.length === 0) {
      notify('Nenhum equipamento com acesso cadastrado.', 'warning')
      return
    }
    runDialog.fleetAction = fleetAction
    void start(
      ready.map((member) => member.deviceId),
      {},
      false
    )
    return
  }
  runDialog.fleetAction = fleetAction
  runDialog.action = action
  runDialog.open = true
}

async function start(deviceIds: number[], params: Record<string, unknown>, confirmWrite: boolean) {
  const fleetAction = runDialog.fleetAction
  if (!fleetAction) return
  live.title = fleetAction.title
  live.batchId = null
  live.open = fleetAction.id !== spec.value?.statusAction
  try {
    live.batchId = await store.runAction(
      pluginId.value,
      fleetAction.id,
      deviceIds,
      confirmWrite,
      params
    )
  } catch (err: unknown) {
    live.open = false
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
</script>
