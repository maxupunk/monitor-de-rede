<template>
  <v-form ref="form" :disabled="disabled" @submit.prevent="emit('submit')">
    <v-select
      v-if="!lockKind"
      v-model="kind"
      :items="KINDS"
      label="Tipo de acesso"
      variant="outlined"
      density="comfortable"
      class="mt-2 mb-3"
    ></v-select>
    <v-row dense>
      <v-col cols="12" sm="7">
        <v-text-field
          v-model="username"
          :label="dedicated ? 'Usuário administrador' : 'Usuário'"
          :rules="[(v: string) => Boolean(v?.trim()) || 'Informe o usuário']"
          variant="outlined"
          density="comfortable"
          autocomplete="off"
          prepend-inner-icon="mdi-account-outline"
        ></v-text-field>
      </v-col>
      <v-col cols="12" sm="5">
        <v-text-field
          v-model.number="port"
          label="Porta"
          type="number"
          :placeholder="String(defaultPort)"
          :hint="`Vazio usa a ${defaultPort}`"
          persistent-hint
          variant="outlined"
          density="comfortable"
        ></v-text-field>
      </v-col>
    </v-row>

    <v-radio-group
      v-if="kind === 'ssh'"
      v-model="storage"
      color="primary"
      class="mt-1 mb-1"
      hide-details
    >
      <v-radio value="key" class="mb-2">
        <template #label>
          <div>
            <div class="font-weight-medium d-flex align-center flex-wrap ga-2">
              Acesso dedicado do NetMonitor
              <v-chip v-if="dedicatedPlatform" color="success" size="x-small" variant="flat">
                Recomendado
              </v-chip>
            </div>
            <div class="text-body-small">
              A senha do administrador é usada uma vez e não é guardada.
              {{ dedicatedAccessEffect(platform ?? dedicatedPlatform) }}
            </div>
          </div>
        </template>
      </v-radio>
      <v-radio value="ask">
        <template #label>
          <div>
            <div class="font-weight-medium">Pedir a senha quando precisar</div>
            <div class="text-body-small">{{ ASK_DESCRIPTION }}</div>
          </div>
        </template>
      </v-radio>
    </v-radio-group>
    <p v-else class="text-body-2 mt-1 mb-1">{{ ASK_DESCRIPTION }}</p>

    <v-select
      v-if="dedicated"
      v-model="platform"
      :items="PLATFORMS"
      label="Sistema do equipamento"
      variant="outlined"
      density="comfortable"
      class="mt-4"
      hide-details
    ></v-select>

    <v-text-field
      v-model="secret"
      :label="dedicated ? 'Senha do administrador' : 'Senha'"
      :type="showSecret ? 'text' : 'password'"
      :append-inner-icon="showSecret ? 'mdi-eye-off' : 'mdi-eye'"
      :hint="secretHint"
      :rules="[secretRule]"
      persistent-hint
      variant="outlined"
      density="comfortable"
      autocomplete="new-password"
      prepend-inner-icon="mdi-lock-outline"
      class="mt-4 mb-3"
      @click:append-inner="showSecret = !showSecret"
    ></v-text-field>

    <v-switch
      v-if="kind === 'http'"
      v-model="https"
      color="primary"
      label="HTTPS"
      inset
      hide-details
      class="mb-2"
    ></v-switch>

    <v-select
      v-model="viaProbeId"
      :items="routes"
      item-title="title"
      item-value="value"
      item-props="props"
      label="Acessar a partir de"
      hint="Equipamento em outra rede? Escolha o agente remoto daquele site."
      persistent-hint
      variant="outlined"
      density="comfortable"
      class="mt-2"
    ></v-select>
  </v-form>
</template>

<script setup lang="ts">
/**
 * O formulário do acesso (credencial) de um equipamento — o único do sistema.
 * Quem o mostra é o `DeviceAccessDialog`; as regras de leitura moram em
 * `utils/deviceCredentials`.
 */
import { computed, ref, watch } from 'vue'
import type { AgentRouteOption } from '@/bindings/AgentRouteOption'
import type { CredentialView } from '@/bindings/CredentialView'
import type { DedicatedPlatform } from '@/bindings/DedicatedPlatform'
import type { SecretStorage } from '@/bindings/SecretStorage'
import type { TransportKind } from '@/bindings/TransportKind'
import { agentRouteItems } from '@/utils/agentRoutes'
import {
  type AccessSubmission,
  credentialOf,
  dedicatedAccessEffect,
  dedicatedPlatformLabel,
  transportLabel,
} from '@/utils/deviceCredentials'

const props = defineProps<{
  /** As credenciais que o equipamento já tem: a do tipo escolhido preenche o formulário. */
  credentials: CredentialView[]
  agents: AgentRouteOption[]
  /** O sistema conhecido do equipamento, quando o acesso dedicado o cobre. */
  dedicatedPlatform?: DedicatedPlatform | null
  /** O tipo vem de quem pediu (o que falta) — não se troca. */
  lockKind?: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{ submit: [] }>()

const kind = defineModel<TransportKind>('kind', { required: true })

const KINDS = (['ssh', 'http', 'telnet'] as const).map((value) => ({
  title: transportLabel(value),
  value,
}))

const PLATFORMS: { title: string; value: DedicatedPlatform | null }[] = [
  { title: 'Descobrir sozinho', value: null },
  ...(['mikrotik', 'openwrt'] as const).map((value) => ({
    title: dedicatedPlatformLabel(value),
    value,
  })),
]

const form = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const username = ref('')
const secret = ref('')
const showSecret = ref(false)
const storage = ref<SecretStorage>('ask')
const port = ref<number | ''>('')
const https = ref(false)
const viaProbeId = ref<number | null>(null)
const platform = ref<DedicatedPlatform | null>(null)

const dedicated = computed(() => kind.value === 'ssh' && storage.value === 'key')

const existing = computed(() => credentialOf(props.credentials, kind.value))

const defaultPort = computed(() => {
  if (kind.value === 'http') return https.value ? 443 : 80
  return kind.value === 'telnet' ? 23 : 22
})

const ASK_DESCRIPTION =
  'A senha não é gravada: é pedida nesta janela quando faz falta e vale 30 minutos.'

const secretHint = computed(() => {
  if (dedicated.value) return 'Usada só para criar o acesso; não fica guardada em lugar nenhum.'
  if (kind.value === 'http') return 'Página sem login? Deixe vazio.'
  if (existing.value?.sessionActive) return 'Já informada; vazio mantém a sessão atual.'
  return 'Libera o acesso por 30 minutos.'
})

/** SSH/Telnet sem senha não entram no equipamento — é o que se veio pedir. */
function secretRule(value: string): boolean | string {
  if (dedicated.value) return Boolean(value) || 'Informe a senha do administrador'
  if (kind.value === 'http' || existing.value?.sessionActive || value) return true
  return 'Informe a senha para liberar o acesso'
}

const routes = computed(() =>
  agentRouteItems(props.agents, 'device_io', 'O equipamento é alcançável daqui')
)

/** Preenche com a credencial do tipo (ou os padrões) — ao abrir e ao trocar o tipo. */
function fill() {
  const current = existing.value
  const suggested = kind.value === 'ssh' ? props.dedicatedPlatform : null
  platform.value = suggested ?? null
  username.value =
    current?.username ?? (suggested === 'mikrotik' ? 'admin' : kind.value === 'ssh' ? 'root' : '')
  storage.value = current?.storage ?? (suggested ? 'key' : 'ask')
  port.value = current?.port ?? ''
  https.value = current?.https ?? false
  viaProbeId.value = current?.viaProbeId ?? null
  secret.value = ''
  showSecret.value = false
}

watch([kind, () => props.credentials], fill, { immediate: true })

/** O acesso dedicado só existe no SSH: trocar de tipo volta para a senha da janela. */
watch(kind, (value) => {
  if (value !== 'ssh' && storage.value === 'key') storage.value = 'ask'
})

/** Confere e monta o pedido; `null` quando há campo a corrigir. */
async function build(): Promise<AccessSubmission | null> {
  const result = await form.value?.validate()
  if (result && !result.valid) return null
  const chosenPort = typeof port.value === 'number' && port.value > 0 ? port.value : undefined
  if (dedicated.value) {
    return {
      mode: 'dedicated',
      input: {
        platform: platform.value ?? undefined,
        username: username.value.trim(),
        password: secret.value,
        port: chosenPort,
        viaProbeId: viaProbeId.value,
      },
    }
  }
  return {
    mode: 'credential',
    input: {
      kind: kind.value,
      username: username.value.trim(),
      secret: secret.value || undefined,
      storage: storage.value,
      port: chosenPort,
      https: kind.value === 'http' ? https.value : undefined,
      viaProbeId: viaProbeId.value,
    },
  }
}

defineExpose({ build, dedicated })
</script>
