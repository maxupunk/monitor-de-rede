<template>
  <v-dialog
    :model-value="modelValue"
    :fullscreen="xs"
    max-width="560"
    scrollable
    @update:model-value="(open) => !open && close()"
  >
    <v-card class="rounded-lg">
      <v-card-item>
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-key-variant</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">{{ title }}</v-card-title>
        <v-card-subtitle v-if="deviceName" class="subtitle">{{ deviceName }}</v-card-subtitle>
      </v-card-item>

      <v-card-text>
        <v-alert
          v-if="reason"
          type="info"
          variant="tonal"
          density="compact"
          icon="mdi-information-outline"
          class="mb-4"
        >
          {{ reason }}
        </v-alert>

        <div v-if="loading" class="d-flex align-center ga-3 py-6 justify-center">
          <v-progress-circular indeterminate color="primary" size="24"></v-progress-circular>
          <span class="text-body-medium">Carregando o acesso do equipamento…</span>
        </div>
        <DeviceAccessForm
          v-else
          ref="form"
          v-model:kind="kind"
          :credentials="credentials"
          :agents="agents"
          :lock-kind="transport !== null"
          :disabled="saving"
          @submit="save"
        />

        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mt-3">
          {{ error }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-4 pb-4 flex-wrap ga-2">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" :disabled="saving" @click="close">
          {{ reason ? 'Agora não' : 'Cancelar' }}
        </v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-content-save-outline"
          :loading="saving"
          :disabled="loading"
          @click="save"
        >
          Salvar acesso
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * O diálogo do acesso do equipamento: carrega o que ele já tem, mostra o
 * formulário único (`DeviceAccessForm`) no transporte que falta e grava pela
 * store. Quem o abre de qualquer tela é o `useDeviceAccess`.
 */
import { computed, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import type { AgentRouteOption } from '@/bindings/AgentRouteOption'
import type { CredentialInput } from '@/bindings/CredentialInput'
import type { CredentialView } from '@/bindings/CredentialView'
import type { TransportKind } from '@/bindings/TransportKind'
import { usePluginsStore } from '@/stores/plugins'
import { accessTitle } from '@/utils/deviceCredentials'
import DeviceAccessForm from './DeviceAccessForm.vue'

const props = defineProps<{
  modelValue: boolean
  deviceId: number
  /** O transporte que falta; `null` deixa escolher. */
  transport: TransportKind | null
  /** Por que o acesso é pedido agora. */
  reason?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: [credential: CredentialView]
}>()

const { xs } = useDisplay()
const store = usePluginsStore()

const form = ref<{ build: () => Promise<CredentialInput | null> } | null>(null)
const kind = ref<TransportKind>('ssh')
const credentials = ref<CredentialView[]>([])
const agents = ref<AgentRouteOption[]>([])
const deviceName = ref('')
const loading = ref(false)
const saving = ref(false)
const error = ref<string | null>(null)
let loadSeq = 0

const title = computed(() =>
  props.transport ? accessTitle(props.transport) : 'Acesso ao equipamento'
)

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

async function load() {
  const seq = ++loadSeq
  loading.value = true
  error.value = null
  kind.value = props.transport ?? 'ssh'
  try {
    const access = await store.fetchAccess(props.deviceId)
    if (seq !== loadSeq) return
    credentials.value = access.credentials
    agents.value = access.agents
    deviceName.value = access.deviceName
  } catch (err: unknown) {
    if (seq === loadSeq) error.value = describe(err, 'Não foi possível carregar o acesso.')
  } finally {
    if (seq === loadSeq) loading.value = false
  }
}

watch(
  () => [props.modelValue, props.deviceId, props.transport] as const,
  ([open]) => {
    if (open) void load()
  },
  { immediate: true }
)

async function save() {
  const input = await form.value?.build()
  if (!input) return
  saving.value = true
  error.value = null
  try {
    const saved = await store.saveCredential(props.deviceId, input)
    emit('saved', saved)
    emit('update:modelValue', false)
  } catch (err: unknown) {
    error.value = describe(err, 'Não foi possível salvar o acesso.')
  } finally {
    saving.value = false
  }
}

function close() {
  loadSeq += 1
  emit('update:modelValue', false)
}
</script>

<style scoped>
.subtitle {
  opacity: 1;
  color: rgb(var(--v-theme-on-surface));
}
</style>
