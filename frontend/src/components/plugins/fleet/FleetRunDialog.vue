<template>
  <v-dialog :model-value="modelValue" max-width="760" scrollable @update:model-value="close">
    <v-card v-if="fleetAction" class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2">
        <v-icon :color="effect.color">{{ fleetAction.icon ?? effect.icon }}</v-icon>
        {{ fleetAction.title }}
      </v-card-title>
      <v-card-subtitle
        >{{ pluginName }} · {{ selected.length }} de {{ members.length }}</v-card-subtitle
      >

      <v-card-text>
        <p v-if="fleetAction.description" class="text-body-2 mb-3">
          {{ fleetAction.description }}
        </p>

        <!-- O que os equipamentos têm agora: base do formulário -->
        <v-alert
          v-if="needsContext"
          :type="statusBatch ? 'info' : 'warning'"
          variant="tonal"
          density="compact"
          class="mb-3"
        >
          {{
            statusRunning
              ? 'Lendo os equipamentos…'
              : statusBatch
                ? 'Valores lidos dos equipamentos ' +
                  formatRelativeTime(statusBatch.finishedAt) +
                  '.'
                : 'Leia os equipamentos para ver o que eles têm agora e escolher a partir disso.'
          }}
          <template #append>
            <v-btn
              size="small"
              color="primary"
              variant="flat"
              prepend-icon="mdi-refresh"
              :loading="statusRunning"
              @click="readNow"
            >
              {{ statusBatch ? 'Ler de novo' : 'Ler agora' }}
            </v-btn>
          </template>
        </v-alert>
        <FleetCurrentValues
          v-if="current.size > 0 && action?.params"
          :members="members"
          :current="current"
          :schema="action.params"
          :read-at="statusBatch?.finishedAt"
        />

        <div class="d-flex align-center mb-1">
          <span class="text-subtitle-2 font-weight-bold">Equipamentos</span>
          <v-spacer></v-spacer>
          <v-btn size="small" variant="text" color="primary" @click="toggleAll">
            {{ selected.length === ready.length ? 'Nenhum' : 'Todos prontos' }}
          </v-btn>
        </div>
        <v-list density="compact" border class="rounded-lg mb-4">
          <v-list-item
            v-for="member in members"
            :key="member.deviceId"
            :title="member.name"
            :subtitle="[member.ip, member.firmware].filter(Boolean).join(' · ')"
            :disabled="!member.credentialsReady"
            @click="toggle(member.deviceId)"
          >
            <template #prepend>
              <v-checkbox-btn
                :model-value="selected.includes(member.deviceId)"
                :disabled="!member.credentialsReady"
                color="primary"
                @click.stop="toggle(member.deviceId)"
              ></v-checkbox-btn>
            </template>
            <template v-if="!member.credentialsReady" #append>
              <v-chip size="x-small" color="warning" variant="flat">Sem credencial</v-chip>
            </template>
          </v-list-item>
        </v-list>

        <SettingsForm
          v-if="hasParams"
          ref="form"
          :model-value="values"
          :schema="action?.params ?? {}"
          :suggestions="suggestions"
          :only="onlyFields"
          compact
          class="mb-2"
          @update:model-value="onEdit"
        />

        <v-alert
          v-if="fleetAction.reduce"
          type="info"
          variant="tonal"
          density="compact"
          class="mb-3"
        >
          Depois de rodar em todos, o resultado é consolidado aqui na central. Sugestões de ajuste
          só entram na configuração quando você aceitar.
        </v-alert>
        <v-alert v-if="!active" type="warning" variant="tonal" density="compact" class="mb-3">
          Plugin ainda não ativo: cada acesso a cada equipamento vai pedir sua aprovação.
        </v-alert>

        <v-card v-if="fleetAction.preview" border flat class="rounded-lg mb-3">
          <v-card-title class="d-flex align-center ga-2 text-subtitle-1 font-weight-bold">
            <v-icon color="primary">mdi-eye-outline</v-icon>
            Pré-visualização
            <v-spacer></v-spacer>
            <v-btn
              size="small"
              color="primary"
              variant="flat"
              prepend-icon="mdi-eye-outline"
              :loading="previewing"
              :disabled="selected.length === 0"
              @click="runPreview"
            >
              Pré-visualizar
            </v-btn>
          </v-card-title>
          <v-card-text>
            <FleetBatchCard
              v-if="previewBatch"
              :batch="previewBatch"
              :kind="previewAction?.output"
              :labels="previewAction?.labels"
              :can-write="false"
            />
            <div v-else-if="previewing" class="d-flex align-center ga-3">
              <v-progress-circular indeterminate color="primary" size="20" />
              Lendo os equipamentos…
            </div>
            <div v-else class="text-body-2">
              Veja o que muda em cada equipamento antes de aplicar. Nada é alterado.
            </div>
            <v-alert
              v-if="previewError"
              type="error"
              variant="tonal"
              density="compact"
              class="mt-2"
              :text="previewError"
            ></v-alert>
          </v-card-text>
        </v-card>

        <WriteConfirm v-if="action?.effect === 'write'" v-model="confirmWrite">
          Esta ação <strong>altera a configuração de {{ selected.length }} equipamento(s)</strong>.
          Rode antes a pré-visualização; cada equipamento guarda uma cópia e volta atrás sozinho se
          a verificação falhar.
        </WriteConfirm>
      </v-card-text>

      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="secondary" @click="close(false)">Cancelar</v-btn>
        <v-btn
          :color="action?.effect === 'write' ? 'error' : 'primary'"
          variant="flat"
          prepend-icon="mdi-play"
          :disabled="selected.length === 0 || (action?.effect === 'write' && !confirmWrite)"
          @click="submit"
        >
          Executar em {{ selected.length }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { FleetAction } from '@/bindings/FleetAction'
import type { FleetMember } from '@/bindings/FleetMember'
import type { FleetSpec } from '@/bindings/FleetSpec'
import type { PluginAction } from '@/bindings/PluginAction'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { effectPresentation } from '@/utils/pluginPresentation'
import { formatRelativeTime } from '@/utils/formatters'
import { currentByDevice, outputsByDevice, paramsFromItem, sourceIndex } from '@/utils/fleetContext'
import { defaultsOf, fieldsOf, paramsOf } from '@/utils/pluginSettings'
import SettingsForm from '../settings/SettingsForm.vue'
import WriteConfirm from '../WriteConfirm.vue'
import FleetBatchCard from './FleetBatchCard.vue'
import FleetCurrentValues from './FleetCurrentValues.vue'

const props = defineProps<{
  modelValue: boolean
  fleetAction: FleetAction | null
  /** A ação de dispositivo que roda em cada membro. */
  action: PluginAction | null
  members: FleetMember[]
  pluginId: number
  pluginName: string
  active: boolean
  /** A ação de leitura que pré-visualiza esta (`fleetAction.preview`). */
  previewAction?: PluginAction | null
  /** Formulário já preenchido (ex.: a rede escolhida na grade). */
  initialParams?: Record<string, unknown>
  /** Só estes marcados; sem lista, todos os prontos. */
  initialDevices?: number[]
  /** A página da frota: ação de estado e grade (de onde vem o contexto). */
  spec?: FleetSpec | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  run: [deviceIds: number[], params: Record<string, unknown>, confirmWrite: boolean]
}>()

const form = ref<{ validate: () => Promise<boolean> } | null>(null)
const selected = ref<number[]>([])
const values = ref<Record<string, unknown>>({})
const confirmWrite = ref(false)
const store = usePluginAppsStore()
const previewBatchId = ref<number | null>(null)
const previewing = ref(false)
const previewError = ref<string | null>(null)
const previewBatch = computed(() => store.batchById(props.pluginId, previewBatchId.value))

const effect = computed(() => effectPresentation(props.action?.effect ?? 'read'))
const ready = computed(() => props.members.filter((member) => member.credentialsReady))
const readyIds = computed(() => ready.value.map((member) => member.deviceId))
const hasParams = computed(() => fieldsOf(props.action?.params).length > 0)
/** O operador mexeu no formulário: preenchimento automático não sobrescreve. */
const edited = ref(false)

// --- O que os equipamentos têm agora (a última leitura da ação de estado) ---
const statusAction = computed(() => props.spec?.statusAction ?? null)
const statusBatch = computed(() =>
  statusAction.value ? store.latestBatch(props.pluginId, statusAction.value) : null
)
const statusRunning = computed(
  () =>
    store.fleets[props.pluginId]?.batches.some(
      (batch) => batch.action === statusAction.value && batch.status === 'running'
    ) ?? false
)
const outputs = computed(() => outputsByDevice(statusBatch.value))
/** Parâmetros que oferecem os valores dos equipamentos (`source`). */
const sourceParams = computed(() =>
  fieldsOf(props.action?.params)
    .filter((field) => typeof field.schema.source === 'string')
    .map((field) => ({
      name: field.name,
      index: sourceIndex(outputs.value, String(field.schema.source)),
    }))
)
const suggestions = computed(() =>
  Object.fromEntries(sourceParams.value.map((param) => [param.name, [...param.index.keys()]]))
)
/** Como um item da grade vira os parâmetros desta ação (editar/remover). */
const itemAction = computed(
  () =>
    [props.spec?.matrix?.edit, props.spec?.matrix?.remove].find(
      (target) => target?.action === props.fleetAction?.id
    ) ?? null
)
const current = computed(() =>
  props.fleetAction?.current
    ? currentByDevice(outputs.value, props.fleetAction.current)
    : new Map<number, Record<string, unknown>>()
)
/** Com os valores atuais lidos, o formulário só tem o que os equipamentos têm
 * (ex.: sem rádio de 6 GHz, sem os campos de 6 GHz). */
const onlyFields = computed(() => {
  if (current.value.size === 0) return undefined
  const names = new Set<string>()
  for (const values of current.value.values())
    for (const name of Object.keys(values)) names.add(name)
  return [...names]
})
const needsContext = computed(
  () =>
    Boolean(statusAction.value) &&
    (sourceParams.value.length > 0 || current.value.size > 0 || Boolean(props.fleetAction?.current))
)

async function readNow() {
  if (!statusAction.value || readyIds.value.length === 0) return
  await store.runAction(props.pluginId, statusAction.value, readyIds.value, false)
}

function onEdit(next: Record<string, unknown>) {
  edited.value = true
  values.value = next
}

// Escolheu um valor que os equipamentos têm (ex.: uma rede): o formulário vem
// com o que eles têm e só quem o tem fica marcado.
watch(
  () => sourceParams.value.map((param) => values.value[param.name]),
  (now, before) => {
    sourceParams.value.forEach((param, i) => {
      if (now[i] === before?.[i]) return
      const hit = param.index.get(String(now[i] ?? ''))
      if (!hit) return
      if (itemAction.value) {
        values.value = { ...values.value, ...paramsFromItem(itemAction.value, hit.item) }
      }
      selected.value = hit.holders.filter((id) => readyIds.value.includes(id))
    })
  }
)

// Um equipamento só: o formulário mostra o que ele tem. Vários: em branco
// (vazio = manter), para não levar o valor de um aos outros.
watch(
  () => [selected.value.length === 1 ? selected.value[0] : null, current.value] as const,
  ([only]) => {
    if (!props.fleetAction?.current || edited.value) return
    const mine = only === null ? undefined : current.value.get(only)
    values.value = { ...defaultsOf(props.action?.params), ...(mine ?? {}) }
  }
)

watch(
  () => [props.modelValue, props.fleetAction] as const,
  ([open]) => {
    if (!open) return
    confirmWrite.value = false
    edited.value = false
    selected.value = props.initialDevices
      ? readyIds.value.filter((id) => props.initialDevices?.includes(id))
      : [...readyIds.value]
    const only = selected.value.length === 1 ? current.value.get(selected.value[0]!) : undefined
    values.value = {
      ...defaultsOf(props.action?.params),
      ...(only ?? {}),
      ...(props.initialParams ?? {}),
    }
    clearPreview()
  },
  { immediate: true }
)

// A pré-visualização vale para o que foi pedido; mudou o pedido, some.
watch([values, selected], clearPreview, { deep: true })

function clearPreview() {
  previewBatchId.value = null
  previewError.value = null
}

async function runPreview() {
  const preview = props.fleetAction?.preview
  if (!preview || (form.value && !(await form.value.validate()))) return
  previewing.value = true
  previewError.value = null
  try {
    const params = paramsOf(props.action?.params, values.value)
    previewBatchId.value = await store.runAction(
      props.pluginId,
      preview,
      [...selected.value],
      false,
      params
    )
  } catch (err: unknown) {
    previewError.value = err instanceof Error ? err.message : 'Falha ao pré-visualizar'
  } finally {
    previewing.value = false
  }
}

function toggle(deviceId: number) {
  const index = selected.value.indexOf(deviceId)
  if (index >= 0) selected.value.splice(index, 1)
  else selected.value.push(deviceId)
}

function toggleAll() {
  selected.value =
    selected.value.length === ready.value.length ? [] : ready.value.map((member) => member.deviceId)
}

function close(value = false) {
  emit('update:modelValue', value)
}

async function submit() {
  if (form.value && !(await form.value.validate())) return
  const params = paramsOf(props.action?.params, values.value)
  emit('run', [...selected.value], params, confirmWrite.value)
  close(false)
}
</script>
