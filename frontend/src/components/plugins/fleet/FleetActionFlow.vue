<template>
  <v-dialog :model-value="modelValue" max-width="760" scrollable @update:model-value="close">
    <v-card v-if="request" class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2 pt-4">
        <v-icon :color="isWrite ? 'primary' : 'secondary'">
          {{ request.fleetAction.icon ?? 'mdi-cog-outline' }}
        </v-icon>
        <span class="text-truncate">{{ request.title }}</span>
      </v-card-title>
      <v-card-subtitle class="pb-2">{{ stepText }}</v-card-subtitle>

      <v-card-text>
        <!-- 1. O pedido -->
        <template v-if="step === 'form'">
          <p v-if="request.fleetAction.description" class="text-body-2 mb-4">
            {{ request.fleetAction.description }}
          </p>

          <v-alert
            v-if="needsContext"
            :type="statusBatch ? 'info' : 'warning'"
            variant="tonal"
            density="compact"
            class="mb-4"
          >
            {{
              statusRunning
                ? 'Lendo os equipamentos…'
                : statusBatch
                  ? 'Valores lidos ' + formatRelativeTime(statusBatch.finishedAt) + '.'
                  : 'Leia os equipamentos para partir do que eles têm agora.'
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
            v-if="current.size > 0 && request.action.params"
            :members="selectedMembers"
            :current="current"
            :schema="request.action.params"
            :read-at="statusBatch?.finishedAt"
          />

          <SettingsForm
            v-if="hasFields"
            ref="form"
            :model-value="values"
            :schema="request.action.params ?? {}"
            :suggestions="suggestions"
            :only="onlyFields"
            compact
            class="mb-2"
            @update:model-value="onEdit"
          />

          <!-- Onde aplicar -->
          <div class="d-flex align-center flex-wrap ga-2 mt-3 mb-1">
            <v-icon size="18" color="primary">mdi-router-network</v-icon>
            <span class="text-subtitle-2 font-weight-bold">{{ targetText }}</span>
            <v-spacer></v-spacer>
            <v-btn size="small" variant="text" color="primary" @click="choosing = !choosing">
              {{ choosing ? 'Ocultar' : 'Escolher' }}
            </v-btn>
          </div>
          <v-expand-transition>
            <v-list v-if="choosing" density="compact" border class="rounded-lg mb-3">
              <v-list-item
                v-for="member in members"
                :key="member.deviceId"
                :title="member.name"
                :subtitle="member.credentialsReady ? (member.ip ?? '') : 'Sem acesso cadastrado'"
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
              </v-list-item>
            </v-list>
          </v-expand-transition>

          <v-alert v-if="!active" type="warning" variant="tonal" density="compact" class="mt-3">
            Plugin ainda não ativo: cada acesso a cada equipamento vai pedir sua aprovação.
          </v-alert>
          <WriteConfirm
            v-if="isWrite && !request.previewAction"
            v-model="confirmWrite"
            class="mt-3"
          />
        </template>

        <!-- 2. Revisar as mudanças (a confirmação de quem altera equipamento) -->
        <template v-else-if="step === 'review'">
          <v-alert
            v-if="previewBatch && previewDone && nothingToChange"
            type="success"
            variant="tonal"
            class="mb-3"
          >
            Nada a mudar: os equipamentos já estão assim.
          </v-alert>
          <v-alert v-else type="info" variant="tonal" density="compact" class="mb-3">
            Confira o que muda em cada equipamento. Nada foi alterado ainda — só ao clicar em
            <strong>Aplicar agora</strong>. Cada um guarda uma cópia e volta atrás sozinho se algo
            der errado.
          </v-alert>
          <FleetBatchCard
            v-if="previewBatch"
            :batch="previewBatch"
            :kind="request.previewAction?.output"
            :presentation="request.previewAction"
            :can-write="false"
            expand-all
          />
          <div v-else class="d-flex align-center ga-3 py-4">
            <v-progress-circular indeterminate color="primary" size="22" />
            Lendo os equipamentos…
          </div>
        </template>

        <!-- 3. Executando / 4. Pronto -->
        <template v-else>
          <v-alert
            v-if="step === 'done' && runBatch"
            :type="outcome.type"
            variant="tonal"
            class="mb-3"
            :title="outcome.title"
            :text="outcome.text"
          ></v-alert>
          <FleetBatchCard
            v-if="runBatch"
            :batch="runBatch"
            :kind="shownAction?.output"
            :presentation="shownAction"
            :result-presentation="runAction ? undefined : request.fleetAction"
            :can-write="false"
            :expand-all="step === 'running'"
            @open-run="(device) => emit('openRun', device)"
          />
          <div v-else class="d-flex align-center ga-3 py-4">
            <v-progress-circular indeterminate color="primary" size="22" />
            Iniciando nos equipamentos…
          </div>

          <!-- Sugestão do consolidado: aceitar é rodar a ação dela -->
          <v-card
            v-if="step === 'done' && followUp && canWrite"
            border
            flat
            class="rounded-lg mt-3"
          >
            <v-card-text class="d-flex align-center flex-wrap ga-3">
              <v-icon color="success">mdi-lightbulb-on-outline</v-icon>
              <span class="text-body-2 flex-grow-1">
                Aplicar a sugestão em {{ Object.keys(followUp.devices).length }} equipamento(s)?
                Isso altera a configuração deles.
              </span>
              <v-btn color="success" variant="flat" prepend-icon="mdi-check" @click="applyFollowUp">
                {{ followUp.title }}
              </v-btn>
            </v-card-text>
          </v-card>
        </template>

        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mt-3">
          {{ error }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-4 pb-4">
        <template v-if="step === 'form'">
          <v-btn variant="text" color="secondary" @click="close(false)">Cancelar</v-btn>
          <v-spacer></v-spacer>
          <v-btn
            v-if="isWrite && request.previewAction"
            color="primary"
            variant="flat"
            append-icon="mdi-arrow-right"
            :disabled="selected.length === 0"
            :loading="busy"
            @click="review"
          >
            Revisar mudanças
          </v-btn>
          <v-btn
            v-else
            :color="isWrite ? 'error' : 'primary'"
            variant="flat"
            prepend-icon="mdi-play"
            :disabled="selected.length === 0 || (isWrite && !confirmWrite)"
            :loading="busy"
            @click="runFromForm"
          >
            {{ isWrite ? 'Aplicar' : 'Executar' }}
          </v-btn>
        </template>
        <template v-else-if="step === 'review'">
          <v-btn variant="text" color="secondary" prepend-icon="mdi-arrow-left" @click="backToForm">
            Voltar e editar
          </v-btn>
          <v-spacer></v-spacer>
          <v-btn
            color="error"
            variant="flat"
            prepend-icon="mdi-check"
            :disabled="!previewDone || reviewedDevices.length === 0 || nothingToChange"
            :loading="busy"
            @click="applyReviewed"
          >
            Aplicar agora
          </v-btn>
        </template>
        <template v-else>
          <v-spacer></v-spacer>
          <v-btn color="primary" variant="flat" @click="close(false)">
            {{ step === 'running' ? 'Continuar em segundo plano' : 'Fechar' }}
          </v-btn>
        </template>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { BatchDevice } from '@/bindings/BatchDevice'
import type { FleetMember } from '@/bindings/FleetMember'
import type { FleetSpec } from '@/bindings/FleetSpec'
import type { PluginAction } from '@/bindings/PluginAction'
import { usePluginAppsStore } from '@/stores/pluginApps'
import {
  currentByDevice,
  outputsByDevice,
  sourceIndex,
  type FlowRequest,
} from '@/utils/fleetContext'
import { paramsFromItem } from '@/utils/itemList'
import { formatRelativeTime } from '@/utils/formatters'
import { followUpOf } from '@/utils/pluginPresentation'
import { acceptedValues, defaultsOf, fieldsOf, paramsOf } from '@/utils/pluginSettings'
import SettingsForm from '../settings/SettingsForm.vue'
import WriteConfirm from '../WriteConfirm.vue'
import FleetBatchCard from './FleetBatchCard.vue'
import FleetCurrentValues from './FleetCurrentValues.vue'

type Step = 'form' | 'review' | 'running' | 'done'

const props = defineProps<{
  modelValue: boolean
  request: FlowRequest | null
  members: FleetMember[]
  pluginId: number
  active: boolean
  canWrite: boolean
  spec?: FleetSpec | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  /** Uma escrita terminou: a tela relê os equipamentos. */
  changed: []
  openRun: [device: BatchDevice]
}>()

const store = usePluginAppsStore()
const form = ref<{ validate: () => Promise<boolean> } | null>(null)
const step = ref<Step>('form')
const selected = ref<number[]>([])
const values = ref<Record<string, unknown>>({})
const edited = ref(false)
const choosing = ref(false)
const confirmWrite = ref(false)
const busy = ref(false)
const error = ref<string | null>(null)
const previewBatchId = ref<number | null>(null)
const runBatchId = ref<number | null>(null)
/** A ação do lote em andamento (a sugestão aceita roda outra ação). */
const runAction = ref<PluginAction | null>(null)

const ready = computed(() => props.members.filter((member) => member.credentialsReady))
const readyIds = computed(() => ready.value.map((member) => member.deviceId))
const selectedMembers = computed(() =>
  props.members.filter((member) => selected.value.includes(member.deviceId))
)
/** A ação mostrada: a do pedido, ou a da sugestão aceita depois. */
const shownAction = computed(() => runAction.value ?? props.request?.action ?? null)
const isWrite = computed(() => shownAction.value?.effect === 'write')
const hasFields = computed(() =>
  fieldsOf(props.request?.action.params).some((field) => !field.hidden)
)
const previewBatch = computed(() => store.batchById(props.pluginId, previewBatchId.value))
const runBatch = computed(() => store.batchById(props.pluginId, runBatchId.value))
const previewDone = computed(() => Boolean(previewBatch.value?.finishedAt))
const followUp = computed(() => followUpOf(runBatch.value?.result))

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
const sourceParams = computed(() =>
  fieldsOf(props.request?.action.params)
    .filter((field) => typeof field.schema.source === 'string')
    .map((field) => ({
      name: field.name,
      index: sourceIndex(outputs.value, String(field.schema.source)),
    }))
)
const suggestions = computed(() =>
  Object.fromEntries(sourceParams.value.map((param) => [param.name, [...param.index.keys()]]))
)
const itemAction = computed(
  () =>
    [
      props.spec?.list?.edit,
      props.spec?.list?.remove,
      ...(props.spec?.list?.rowActions ?? []),
    ].find((target) => target?.action === props.request?.fleetAction.id) ?? null
)
const current = computed(() =>
  props.request?.fleetAction.current
    ? currentByDevice(outputs.value, props.request.fleetAction.current)
    : new Map<number, Record<string, unknown>>()
)
/** Só os campos que os equipamentos têm (ex.: sem rádio de 6 GHz, sem 6 GHz). */
const onlyFields = computed(() => {
  if (current.value.size === 0) return undefined
  const names = new Set<string>()
  for (const entry of current.value.values()) for (const name of Object.keys(entry)) names.add(name)
  return [...names]
})
const needsContext = computed(
  () =>
    Boolean(statusAction.value) &&
    (sourceParams.value.length > 0 || Boolean(props.request?.fleetAction.current))
)

/** Quem a revisão aprovou: os que responderam (os que falharam ficam de fora). */
const reviewedDevices = computed(
  () =>
    previewBatch.value?.devices
      .filter((device) => device.status === 'succeeded')
      .map((device) => device.deviceId) ?? []
)
/** A revisão diz que nada muda em nenhum equipamento. */
const nothingToChange = computed(() => {
  const devices = previewBatch.value?.devices ?? []
  return (
    devices.length > 0 &&
    devices.every((device) => {
      const output = device.output
      return (
        device.status === 'succeeded' &&
        typeof output === 'object' &&
        output !== null &&
        'changes' in output &&
        Array.isArray(output.changes) &&
        output.changes.length === 0
      )
    })
  )
})

const targetText = computed(() => {
  if (selected.value.length === 0) return 'Nenhum equipamento escolhido'
  if (selected.value.length === ready.value.length && ready.value.length > 1) {
    return `Em todos os equipamentos (${selected.value.length})`
  }
  return 'Em ' + selectedMembers.value.map((member) => member.name).join(', ')
})

const stepText = computed(() => {
  switch (step.value) {
    case 'form':
      return isWrite.value && props.request?.previewAction
        ? 'Passo 1 de 2 — o que mudar'
        : 'Confira e execute'
    case 'review':
      return 'Passo 2 de 2 — revise e aplique'
    case 'running':
      return 'Em andamento…'
    default:
      return 'Concluído'
  }
})

const outcome = computed(() => {
  const devices = runBatch.value?.devices ?? []
  const ok = devices.filter((device) => device.status === 'succeeded').length
  const failed = devices.length - ok
  if (runBatch.value?.status === 'cancelled') {
    return {
      type: 'warning' as const,
      title: 'Cancelado',
      text: `${ok} de ${devices.length} concluídos.`,
    }
  }
  if (failed === 0) {
    return {
      type: 'success' as const,
      title: 'Pronto!',
      text: isWrite.value
        ? `Aplicado em ${ok} equipamento(s).`
        : `Concluído em ${ok} equipamento(s).`,
    }
  }
  return {
    type: (ok > 0 ? 'warning' : 'error') as 'warning' | 'error',
    title: ok > 0 ? 'Concluído com falhas' : 'Não deu certo',
    text: `${ok} de ${devices.length} equipamento(s) concluídos. Veja abaixo o que houve em cada um.`,
  }
})

// Abrir: tudo do zero; ação de leitura sem nada a perguntar já roda.
watch(
  () => [props.modelValue, props.request] as const,
  ([open, request]) => {
    if (!open || !request) return
    step.value = 'form'
    error.value = null
    edited.value = false
    confirmWrite.value = false
    previewBatchId.value = null
    runBatchId.value = null
    runAction.value = null
    selected.value = request.initialDevices
      ? readyIds.value.filter((id) => request.initialDevices?.includes(id))
      : [...readyIds.value]
    choosing.value = selected.value.length !== readyIds.value.length
    values.value = startingValues()
    if (request.action.effect === 'read' && !hasFields.value) void runFromForm()
  },
  { immediate: true }
)

function startingValues(): Record<string, unknown> {
  const schema = props.request?.action.params
  const only = selected.value.length === 1 ? current.value.get(selected.value[0]!) : undefined
  return {
    ...defaultsOf(schema),
    ...(only ? acceptedValues(schema, only) : {}),
    ...(props.request?.initialParams ?? {}),
  }
}

function onEdit(next: Record<string, unknown>) {
  edited.value = true
  values.value = next
}

// Escolheu um valor que os equipamentos têm: o formulário vem com o que eles
// têm e só quem o tem fica marcado.
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

// Um equipamento só: o formulário mostra o que ele tem; vários: em branco
// (vazio = manter), para o valor de um não ir para os outros.
watch(
  () => [selected.value.length === 1 ? selected.value[0] : null, current.value] as const,
  () => {
    if (!props.request?.fleetAction.current || edited.value || step.value !== 'form') return
    values.value = startingValues()
  }
)

// O lote terminou: passo final; escrita avisa a tela para reler.
watch(
  () => runBatch.value?.finishedAt,
  (finished) => {
    if (!finished || step.value !== 'running') return
    step.value = 'done'
    if (isWrite.value) emit('changed')
  }
)

function toggle(deviceId: number) {
  const index = selected.value.indexOf(deviceId)
  if (index >= 0) selected.value.splice(index, 1)
  else selected.value.push(deviceId)
}

async function readNow() {
  if (!statusAction.value || readyIds.value.length === 0) return
  await store.runAction(props.pluginId, statusAction.value, readyIds.value, false)
}

function params(): Record<string, unknown> {
  return paramsOf(props.request?.action.params, values.value)
}

async function attempt(work: () => Promise<void>) {
  busy.value = true
  error.value = null
  try {
    await work()
  } catch (err: unknown) {
    error.value = err instanceof Error ? err.message : 'Não foi possível iniciar'
  } finally {
    busy.value = false
  }
}

async function review() {
  const preview = props.request?.fleetAction.preview
  if (!preview || (form.value && !(await form.value.validate()))) return
  await attempt(async () => {
    previewBatchId.value = await store.runAction(
      props.pluginId,
      preview,
      [...selected.value],
      false,
      params()
    )
    step.value = 'review'
  })
}

function backToForm() {
  previewBatchId.value = null
  step.value = 'form'
}

async function start(
  fleetActionId: string,
  deviceIds: number[],
  confirm: boolean,
  common: Record<string, unknown>,
  deviceParams?: Record<number, Record<string, unknown>>
) {
  await attempt(async () => {
    runBatchId.value = await store.runAction(
      props.pluginId,
      fleetActionId,
      deviceIds,
      confirm,
      common,
      deviceParams
    )
    step.value = 'running'
  })
}

async function runFromForm() {
  if (!props.request || (form.value && !(await form.value.validate()))) return
  await start(props.request.fleetAction.id, [...selected.value], confirmWrite.value, params())
}

async function applyReviewed() {
  if (!props.request) return
  await start(props.request.fleetAction.id, reviewedDevices.value, true, params())
}

/** Aceitar a sugestão: a ação dela, com o de cada equipamento (o botão é a confirmação). */
async function applyFollowUp() {
  const next = followUp.value
  if (!next) return
  const fleetAction = props.spec?.actions?.find((action) => action.id === next.action)
  runAction.value =
    store.fleets[props.pluginId]?.plugin.actions.find(
      (action) => action.id === fleetAction?.action
    ) ?? null
  await start(
    next.action,
    Object.keys(next.devices).map(Number),
    runAction.value?.effect === 'write',
    {},
    next.devices
  )
}

function close(value = false) {
  emit('update:modelValue', value)
}
</script>
