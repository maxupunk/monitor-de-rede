<template>
  <div>
    <v-card v-if="canWrite" border flat class="rounded-lg mb-4">
      <v-card-text class="d-flex flex-wrap align-center ga-3">
        <v-autocomplete
          v-model="candidateId"
          :items="candidateItems"
          item-props
          label="Adicionar equipamento"
          no-data-text="Nenhum outro equipamento com IP cadastrado"
          variant="outlined"
          density="compact"
          hide-details
          class="flex-grow-1"
          style="min-width: 240px"
        ></v-autocomplete>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-plus"
          :disabled="candidateId === null"
          :loading="adding"
          @click="add"
        >
          Adicionar
        </v-btn>
        <v-btn
          color="secondary"
          variant="flat"
          prepend-icon="mdi-plus-network-outline"
          @click="emit('createDevice')"
        >
          Cadastrar novo
        </v-btn>
        <v-btn
          v-if="doubtful > 0"
          color="primary"
          variant="flat"
          prepend-icon="mdi-magnify-scan"
          :loading="store.identifying"
          @click="identify"
        >
          Verificar sistema ({{ doubtful }})
        </v-btn>
      </v-card-text>
    </v-card>

    <v-alert v-if="view.members.length === 0" type="info" variant="tonal" class="mb-4">
      Nenhum equipamento ainda. Adicione um já cadastrado ou cadastre um novo acima.
    </v-alert>

    <v-row dense>
      <v-col v-for="member in view.members" :key="member.deviceId" cols="12" md="6" xl="4">
        <v-card
          border
          flat
          class="rounded-lg h-100"
          :link="canConfigure(member)"
          @click="canConfigure(member) && emit('configure', member.deviceId)"
        >
          <v-card-item>
            <template #prepend>
              <v-avatar :color="statusOf(member).color" variant="tonal" rounded="lg">
                <v-icon>mdi-router-wireless</v-icon>
              </v-avatar>
            </template>
            <v-card-title class="font-weight-bold">{{ member.name }}</v-card-title>
            <v-card-subtitle>
              {{ [member.ip, member.firmware].filter(Boolean).join(' · ') }}
            </v-card-subtitle>
            <template #append>
              <v-chip size="small" :color="statusOf(member).color" variant="flat" class="mr-1">
                {{ statusOf(member).label }}
              </v-chip>
              <v-menu>
                <template #activator="{ props: menu }">
                  <v-btn
                    v-bind="menu"
                    icon="mdi-dots-vertical"
                    size="small"
                    variant="text"
                    color="primary"
                    :aria-label="`Opções de ${member.name}`"
                    @click.stop
                  />
                </template>
                <v-list density="compact">
                  <v-list-item
                    v-if="canConfigure(member)"
                    prepend-icon="mdi-tune-variant"
                    :title="deviceActionTitle ?? 'Configurar'"
                    @click="emit('configure', member.deviceId)"
                  ></v-list-item>
                  <v-list-item
                    v-if="deviceSchema && canWrite"
                    prepend-icon="mdi-cog-outline"
                    title="Ajustes guardados"
                    @click="openSettings(member)"
                  ></v-list-item>
                  <v-list-item
                    prepend-icon="mdi-open-in-new"
                    title="Abrir o dispositivo"
                    :to="{ path: '/devices/' + member.deviceId }"
                  ></v-list-item>
                  <v-list-item
                    v-if="canWrite"
                    prepend-icon="mdi-close-circle-outline"
                    base-color="error"
                    title="Tirar deste aplicativo"
                    @click="remove(member)"
                  ></v-list-item>
                </v-list>
              </v-menu>
            </template>
          </v-card-item>

          <v-card-text class="pt-0">
            <div
              v-for="row in cardOf(member)"
              :key="row.label"
              class="d-flex justify-space-between ga-2 text-body-2 py-1"
            >
              <span>{{ row.label }}</span>
              <strong class="text-right">{{ row.value }}</strong>
            </div>
            <div v-if="errorOf(member)" class="text-body-small text-error mt-1">
              {{ errorOf(member) }}
            </div>
            <div
              v-if="member.compat !== 'likely' && member.compat !== 'validated'"
              class="text-body-small mt-2"
            >
              <v-chip
                size="x-small"
                :color="compatPresentation(member.compat).color"
                variant="tonal"
              >
                {{ compatPresentation(member.compat).label }}
              </v-chip>
              {{ member.reasons.join('; ') }}
            </div>
            <v-btn
              v-if="!member.credentialsReady"
              size="small"
              color="warning"
              variant="flat"
              prepend-icon="mdi-key-outline"
              class="mt-3"
              :to="{ path: '/devices/' + member.deviceId, query: { tab: 'plugins' } }"
              @click.stop
            >
              Cadastrar o acesso (SSH)
            </v-btn>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <v-dialog v-model="settings.open" max-width="640" scrollable>
      <v-card v-if="settings.member" class="rounded-lg">
        <v-card-title>Ajustes de {{ settings.member.name }}</v-card-title>
        <v-card-subtitle
          >Vale só para este equipamento e sobrepõe a configuração geral.</v-card-subtitle
        >
        <v-card-text>
          <SettingsForm ref="form" v-model="settings.value" :schema="deviceSchema ?? {}" compact />
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" color="secondary" @click="settings.open = false">Cancelar</v-btn>
          <v-btn color="primary" variant="flat" :loading="settings.saving" @click="saveSettings">
            Salvar
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import type { FleetMember } from '@/bindings/FleetMember'
import type { FleetView } from '@/bindings/FleetView'
import { confirm } from '@/composables/useConfirm'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { valueAt } from '@/utils/fleetContext'
import { compatPresentation, stateColor, stateLabel } from '@/utils/pluginPresentation'
import { defaultsOf, normalizeValue } from '@/utils/pluginSettings'
import SettingsForm from '../settings/SettingsForm.vue'

const props = defineProps<{
  view: FleetView
  canWrite: boolean
  /** Nome da ação que o clique no equipamento abre (ex.: "Rádios e canais"). */
  deviceActionTitle?: string
}>()
const emit = defineEmits<{
  notify: [text: string, color: string]
  /** Cadastrar um equipamento que ainda não existe no sistema. */
  createDevice: []
  /** Abrir a ação do equipamento (`fleet.deviceAction`) só para ele. */
  configure: [deviceId: number]
}>()

const store = usePluginAppsStore()
const form = ref<{ validate: () => Promise<boolean> } | null>(null)
const candidateId = ref<number | null>(null)
const adding = ref(false)
const settings = reactive({
  open: false,
  saving: false,
  member: null as FleetMember | null,
  value: {} as Record<string, unknown>,
})

const deviceSchema = computed(() => props.view.plugin.settings?.device ?? null)

// --- O que a última leitura disse de cada equipamento ---
const statusAction = computed(() => props.view.plugin.fleet?.statusAction ?? null)
const statusBatch = computed(() =>
  statusAction.value ? store.latestBatch(props.view.plugin.id, statusAction.value) : null
)

function readOf(member: FleetMember) {
  return statusBatch.value?.devices.find((device) => device.deviceId === member.deviceId)
}

function canConfigure(member: FleetMember): boolean {
  return props.canWrite && Boolean(props.view.plugin.fleet?.deviceAction) && member.credentialsReady
}

function statusOf(member: FleetMember): { label: string; color: string } {
  if (!member.credentialsReady) return { label: 'Sem acesso', color: 'warning' }
  const read = readOf(member)
  if (!read) return { label: 'Não lido', color: 'secondary' }
  if (read.status === 'failed') return { label: 'Sem resposta', color: 'error' }
  const state = valueAt(read.output, 'state')
  return state
    ? { label: stateLabel(state), color: stateColor(state) }
    : { label: 'Lido', color: 'success' }
}

function errorOf(member: FleetMember): string | null {
  const read = readOf(member)
  return read?.status === 'failed' ? read.error : null
}

/** As linhas do cartão (`_card` da leitura: cada rádio, clientes…). */
function cardOf(member: FleetMember): { label: string; value: string }[] {
  const card = valueAt(readOf(member)?.output, '_card')
  if (!Array.isArray(card)) return []
  return card.flatMap((row) => {
    const label = valueAt(row, 'label')
    const value = valueAt(row, 'value')
    return label === undefined || value === undefined
      ? []
      : [{ label: String(label), value: String(value) }]
  })
}
/**
 * Todo equipamento fora da frota, com o veredito e o porquê. O incompatível
 * aparece desabilitado: sumir com ele deixaria o operador sem saber o motivo
 * (ex.: o sistema cadastrado não é o que o plugin atende).
 */
const candidateItems = computed(() =>
  props.view.candidates.map((candidate) => ({
    title: candidate.name + (candidate.ip ? ' — ' + candidate.ip : ''),
    subtitle: `${compatPresentation(candidate.compat).label}: ${candidate.reasons.join('; ')}`,
    value: candidate.deviceId,
    disabled: candidate.compat === 'incompatible',
  }))
)

/** Equipamentos cuja compatibilidade ainda é dúvida (sistema não confirmado). */
const doubtful = computed(
  () =>
    [...props.view.members, ...props.view.candidates].filter(
      (item) => item.compat === 'possible' || item.compat === 'incompatible'
    ).length
)

/**
 * Vai aos equipamentos em dúvida (SSH, SNMP; o Laya se faltar evidência) e
 * refaz a compatibilidade pelo sistema que eles mostrarem — não pelo
 * fabricante do cadastro, que descreve o hardware.
 */
async function identify() {
  try {
    const view = await store.identify(props.view.plugin.id)
    const still = [...view.members, ...view.candidates].filter(
      (item) => item.compat === 'possible'
    ).length
    emit(
      'notify',
      still > 0
        ? `Sistemas verificados. ${still} ainda sem confirmação — defina o sistema no cadastro deles.`
        : 'Sistemas verificados.',
      still > 0 ? 'warning' : 'success'
    )
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao verificar os sistemas'), 'error')
  }
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

async function add() {
  if (candidateId.value === null) return
  adding.value = true
  try {
    await store.addMember(props.view.plugin.id, candidateId.value)
    candidateId.value = null
    emit('notify', 'Equipamento adicionado.', 'success')
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao adicionar'), 'error')
  } finally {
    adding.value = false
  }
}

async function remove(member: FleetMember) {
  const ok = await confirm({
    title: 'Remover equipamento',
    message: `Tirar “${member.name}” deste aplicativo? Nada é alterado no equipamento; as redes já aplicadas continuam nele até você aplicar outra configuração.`,
    confirmText: 'Remover',
    confirmColor: 'error',
  })
  if (!ok) return
  try {
    await store.removeMember(props.view.plugin.id, member.deviceId)
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao remover'), 'error')
  }
}

function openSettings(member: FleetMember) {
  settings.member = member
  settings.value = { ...defaultsOf(deviceSchema.value), ...(member.settings ?? {}) }
  settings.open = true
}

async function saveSettings() {
  const member = settings.member
  if (!member || (form.value && !(await form.value.validate()))) return
  settings.saving = true
  try {
    await store.saveMemberSettings(
      props.view.plugin.id,
      member.deviceId,
      normalizeValue(deviceSchema.value, settings.value)
    )
    settings.open = false
    emit('notify', 'Ajustes salvos. Aplique a configuração para levar ao equipamento.', 'success')
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao salvar'), 'error')
  } finally {
    settings.saving = false
  }
}
</script>
