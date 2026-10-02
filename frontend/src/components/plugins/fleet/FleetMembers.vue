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

    <v-alert type="info" variant="tonal" density="compact" class="mb-4">
      O acesso de cada equipamento (usuário e senha SSH/HTTP) fica no cadastro do próprio
      dispositivo, na aba <strong>Plugins → Credenciais</strong>. Aqui você só escolhe quem
      participa e o ajuste de cada um.
    </v-alert>

    <v-card border flat class="rounded-lg">
      <v-table density="comfortable">
        <thead>
          <tr>
            <th class="font-weight-bold">Equipamento</th>
            <th class="font-weight-bold">Firmware</th>
            <th class="font-weight-bold">Compatibilidade</th>
            <th class="font-weight-bold">Acesso</th>
            <th class="font-weight-bold text-right">Ações</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="view.members.length === 0">
            <td colspan="5" class="text-body-2">Nenhum equipamento ainda.</td>
          </tr>
          <tr v-for="member in view.members" :key="member.deviceId">
            <td>
              <router-link
                :to="{ path: '/devices/' + member.deviceId }"
                class="font-weight-bold text-primary text-decoration-none"
              >
                {{ member.name }}
              </router-link>
              <div class="text-body-small">{{ member.ip ?? '—' }}</div>
            </td>
            <td>{{ member.firmware ?? '—' }}</td>
            <td>
              <v-chip size="small" :color="compatPresentation(member.compat).color" variant="tonal">
                {{ compatPresentation(member.compat).label }}
              </v-chip>
              <div
                v-if="member.compat !== 'likely' && member.compat !== 'validated'"
                class="text-body-small mt-1"
              >
                {{ member.reasons.join('; ') }}
              </div>
            </td>
            <td>
              <v-chip
                size="small"
                :color="member.credentialsReady ? 'success' : 'warning'"
                variant="flat"
              >
                {{ member.credentialsReady ? 'Pronto' : 'Sem credencial' }}
              </v-chip>
            </td>
            <td class="text-right text-no-wrap">
              <v-btn
                v-if="deviceSchema"
                size="small"
                color="primary"
                variant="text"
                prepend-icon="mdi-tune-variant"
                :disabled="!canWrite"
                @click="openSettings(member)"
              >
                Ajustes
              </v-btn>
              <v-btn
                v-if="!member.credentialsReady"
                size="small"
                color="warning"
                variant="text"
                prepend-icon="mdi-key-outline"
                :to="{ path: '/devices/' + member.deviceId, query: { tab: 'plugins' } }"
              >
                Cadastrar acesso
              </v-btn>
              <v-btn
                size="small"
                color="error"
                variant="text"
                icon="mdi-close-circle-outline"
                :disabled="!canWrite"
                :aria-label="`Remover ${member.name}`"
                @click="remove(member)"
              />
            </td>
          </tr>
        </tbody>
      </v-table>
    </v-card>

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
import { compatPresentation } from '@/utils/pluginPresentation'
import { defaultsOf, normalizeValue } from '@/utils/pluginSettings'
import SettingsForm from '../settings/SettingsForm.vue'

const props = defineProps<{ view: FleetView; canWrite: boolean }>()
const emit = defineEmits<{
  notify: [text: string, color: string]
  /** Cadastrar um equipamento que ainda não existe no sistema. */
  createDevice: []
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
