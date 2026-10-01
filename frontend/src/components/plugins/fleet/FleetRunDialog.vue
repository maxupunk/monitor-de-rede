<template>
  <v-dialog :model-value="modelValue" max-width="620" scrollable @update:model-value="close">
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
          v-model="values"
          :schema="action?.params ?? {}"
          compact
          class="mb-2"
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
import type { PluginAction } from '@/bindings/PluginAction'
import { effectPresentation } from '@/utils/pluginPresentation'
import { defaultsOf, fieldsOf, paramsOf } from '@/utils/pluginSettings'
import SettingsForm from '../settings/SettingsForm.vue'
import WriteConfirm from '../WriteConfirm.vue'

const props = defineProps<{
  modelValue: boolean
  fleetAction: FleetAction | null
  /** A ação de dispositivo que roda em cada membro. */
  action: PluginAction | null
  members: FleetMember[]
  pluginName: string
  active: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  run: [deviceIds: number[], params: Record<string, unknown>, confirmWrite: boolean]
}>()

const form = ref<{ validate: () => Promise<boolean> } | null>(null)
const selected = ref<number[]>([])
const values = ref<Record<string, unknown>>({})
const confirmWrite = ref(false)

const effect = computed(() => effectPresentation(props.action?.effect ?? 'read'))
const ready = computed(() => props.members.filter((member) => member.credentialsReady))
const hasParams = computed(() => fieldsOf(props.action?.params).length > 0)

watch(
  () => [props.modelValue, props.fleetAction] as const,
  ([open]) => {
    if (!open) return
    confirmWrite.value = false
    selected.value = ready.value.map((member) => member.deviceId)
    values.value = defaultsOf(props.action?.params)
  },
  { immediate: true }
)

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
