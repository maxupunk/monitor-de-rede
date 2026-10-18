<template>
  <v-form ref="form" :disabled="disabled" @submit.prevent="emit('submit')">
    <v-select
      v-if="!lockKind"
      v-model="kind"
      :items="KINDS"
      label="Tipo de acesso"
      variant="outlined"
      density="comfortable"
      class="mb-3"
    ></v-select>
    <v-row dense>
      <v-col cols="12" sm="7">
        <v-text-field
          v-model="username"
          label="Usuário"
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

    <v-radio-group v-model="storage" color="primary" class="mt-1 mb-1" hide-details>
      <v-radio value="vault">
        <template #label>
          <div>
            <div class="font-weight-medium">Guardar cifrada</div>
            <div class="text-body-small">
              Cifrada com a chave do servidor. Nunca volta para a tela nem para a IA.
            </div>
          </div>
        </template>
      </v-radio>
      <v-radio value="ask" class="mt-2">
        <template #label>
          <div>
            <div class="font-weight-medium">Pedir a cada sessão</div>
            <div class="text-body-small">
              Nada é gravado; a senha vale por 30 minutos depois de informada.
            </div>
          </div>
        </template>
      </v-radio>
    </v-radio-group>

    <v-text-field
      v-model="secret"
      label="Senha"
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
import type { CredentialInput } from '@/bindings/CredentialInput'
import type { CredentialView } from '@/bindings/CredentialView'
import type { SecretStorage } from '@/bindings/SecretStorage'
import type { TransportKind } from '@/bindings/TransportKind'
import { agentRouteItems } from '@/utils/agentRoutes'
import { credentialOf, transportLabel } from '@/utils/deviceCredentials'

const props = defineProps<{
  /** As credenciais que o equipamento já tem: a do tipo escolhido preenche o formulário. */
  credentials: CredentialView[]
  agents: AgentRouteOption[]
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

const form = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const username = ref('')
const secret = ref('')
const showSecret = ref(false)
const storage = ref<SecretStorage>('vault')
const port = ref<number | ''>('')
const https = ref(false)
const viaProbeId = ref<number | null>(null)

const existing = computed(() => credentialOf(props.credentials, kind.value))

const defaultPort = computed(() => {
  if (kind.value === 'http') return https.value ? 443 : 80
  return kind.value === 'telnet' ? 23 : 22
})

const secretHint = computed(() => {
  if (storage.value === 'ask') return 'Opcional agora: se informada, já abre a sessão.'
  if (existing.value?.hasStoredSecret) return 'Deixe vazio para manter a senha guardada.'
  return kind.value === 'http'
    ? 'Página sem login? Deixe vazio.'
    : 'Obrigatória para guardar cifrada.'
})

/** SSH/Telnet guardados sem senha não entram no equipamento — é o que faltava. */
function secretRule(value: string): boolean | string {
  if (storage.value !== 'vault' || kind.value === 'http') return true
  if (existing.value?.hasStoredSecret || value) return true
  return 'Informe a senha para guardar cifrada'
}

const routes = computed(() =>
  agentRouteItems(props.agents, 'device_io', 'O equipamento é alcançável daqui')
)

/** Preenche com a credencial do tipo (ou os padrões) — ao abrir e ao trocar o tipo. */
function fill() {
  const current = existing.value
  username.value = current?.username ?? (kind.value === 'ssh' ? 'root' : '')
  storage.value = current?.storage ?? 'vault'
  port.value = current?.port ?? ''
  https.value = current?.https ?? false
  viaProbeId.value = current?.viaProbeId ?? null
  secret.value = ''
  showSecret.value = false
}

watch([kind, () => props.credentials], fill, { immediate: true })

/** Confere e monta o pedido; `null` quando há campo a corrigir. */
async function build(): Promise<CredentialInput | null> {
  const result = await form.value?.validate()
  if (result && !result.valid) return null
  return {
    kind: kind.value,
    username: username.value.trim(),
    secret: secret.value || undefined,
    storage: storage.value,
    port: typeof port.value === 'number' && port.value > 0 ? port.value : undefined,
    https: kind.value === 'http' ? https.value : undefined,
    viaProbeId: viaProbeId.value,
  }
}

defineExpose({ build })
</script>
