<template>
  <v-dialog :model-value="modelValue" max-width="520" @update:model-value="close">
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2">
        <v-icon color="primary">mdi-key-variant</v-icon>
        {{ existing ? 'Editar credencial' : 'Nova credencial de acesso' }}
      </v-card-title>
      <v-card-text>
        <v-form ref="form" @submit.prevent="submit">
          <v-select
            v-model="kind"
            :items="kinds"
            label="Tipo de acesso"
            :disabled="Boolean(existing)"
            variant="outlined"
            density="comfortable"
            class="mb-3"
          ></v-select>
          <v-text-field
            v-model="username"
            label="Usuário"
            :rules="[(v: string) => Boolean(v?.trim()) || 'Informe o usuário']"
            variant="outlined"
            density="comfortable"
            autocomplete="off"
            class="mb-3"
          ></v-text-field>

          <v-radio-group v-model="storage" color="primary" class="mb-1" hide-details>
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
            type="password"
            :hint="secretHint"
            persistent-hint
            variant="outlined"
            density="comfortable"
            autocomplete="new-password"
            class="mt-4 mb-3"
          ></v-text-field>

          <v-row dense>
            <v-col cols="6">
              <v-text-field
                v-model.number="port"
                label="Porta"
                type="number"
                :placeholder="String(defaultPort)"
                variant="outlined"
                density="comfortable"
              ></v-text-field>
            </v-col>
            <v-col v-if="kind === 'http'" cols="6">
              <v-switch v-model="https" color="primary" label="HTTPS" inset hide-details></v-switch>
            </v-col>
          </v-row>

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
      </v-card-text>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="secondary" @click="close(false)">Cancelar</v-btn>
        <v-btn color="primary" variant="flat" :loading="saving" @click="submit">Salvar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { AgentRouteOption } from '@/bindings/AgentRouteOption'
import { agentRouteItems } from '@/utils/agentRoutes'
import type { CredentialInput } from '@/bindings/CredentialInput'
import type { CredentialView } from '@/bindings/CredentialView'
import type { SecretStorage } from '@/bindings/SecretStorage'
import type { TransportKind } from '@/bindings/TransportKind'

const props = defineProps<{
  modelValue: boolean
  existing: CredentialView | null
  agents: AgentRouteOption[]
  saving?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  save: [input: CredentialInput]
}>()

const kinds = [
  { title: 'SSH', value: 'ssh' },
  { title: 'HTTP (interface web)', value: 'http' },
  { title: 'Telnet', value: 'telnet' },
]

const form = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const kind = ref<TransportKind>('ssh')
const username = ref('')
const secret = ref('')
const storage = ref<SecretStorage>('vault')
const port = ref<number | ''>('')
const https = ref(false)
const viaProbeId = ref<number | null>(null)

const defaultPort = computed(() => {
  if (kind.value === 'http') return https.value ? 443 : 80
  return kind.value === 'telnet' ? 23 : 22
})

const secretHint = computed(() => {
  if (storage.value === 'ask') return 'Opcional agora: se informada, já abre a sessão.'
  if (props.existing?.hasStoredSecret) return 'Deixe vazio para manter a senha guardada.'
  return 'Obrigatória para guardar cifrada.'
})

const routes = computed(() =>
  agentRouteItems(props.agents, 'device_io', 'O equipamento é alcançável daqui')
)

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    const existing = props.existing
    kind.value = existing?.kind ?? 'ssh'
    username.value = existing?.username ?? (kind.value === 'ssh' ? 'root' : '')
    storage.value = existing?.storage ?? 'vault'
    port.value = existing?.port ?? ''
    https.value = existing?.https ?? false
    viaProbeId.value = existing?.viaProbeId ?? null
    secret.value = ''
  }
)

function close(value = false) {
  emit('update:modelValue', value)
}

async function submit() {
  const result = await form.value?.validate()
  if (result && !result.valid) return
  emit('save', {
    kind: kind.value,
    username: username.value.trim(),
    secret: secret.value || undefined,
    storage: storage.value,
    port: typeof port.value === 'number' && port.value > 0 ? port.value : undefined,
    https: kind.value === 'http' ? https.value : undefined,
    viaProbeId: viaProbeId.value,
  })
}
</script>
