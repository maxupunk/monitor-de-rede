<template>
  <v-card elevation="2" class="pa-4">
    <div class="d-flex align-center ga-2 mb-3">
      <div>
        <div class="text-subtitle-1 font-weight-bold">Tokens de acesso pessoal</div>
        <div class="text-body-2">
          Para <code>docker login</code> pela rede interna ou pela VPN. Pela internet o registry só
          aceita a central — nunca um token pessoal.
        </div>
      </div>
      <v-spacer></v-spacer>
      <v-btn color="primary" variant="flat" prepend-icon="mdi-plus" @click="openForm"
        >Novo token</v-btn
      >
    </div>

    <v-alert
      v-if="error && !formOpen"
      type="error"
      variant="tonal"
      density="compact"
      class="mb-3"
      >{{ error }}</v-alert
    >

    <v-alert v-if="created" type="success" variant="tonal" class="mb-4" data-test="token-created">
      <div class="font-weight-bold mb-1">Copie agora: o token não aparece de novo.</div>
      <CopyableCommand label="Token" :command="created.secret" class="mb-2" />
      <div class="text-body-2 mb-1">No terminal, e cole o token quando pedir a senha:</div>
      <CopyableCommand label="Terminal" :command="dockerLogin(host)" />
    </v-alert>

    <v-list v-if="items.length" density="compact" class="bg-transparent pa-0">
      <v-list-item
        v-for="token in items"
        :key="token.id"
        class="px-0"
        :title="token.name"
        :subtitle="subtitle(token)"
      >
        <template #prepend>
          <v-icon :color="token.revoked || token.expired ? 'secondary' : 'primary'" class="mr-3">
            mdi-key-variant
          </v-icon>
        </template>
        <template #append>
          <v-chip v-if="token.revoked" size="small" color="secondary" variant="tonal" label
            >Revogado</v-chip
          >
          <v-chip v-else-if="token.expired" size="small" color="warning" variant="tonal" label
            >Vencido</v-chip
          >
          <IconAction
            v-else
            icon="mdi-cancel"
            label="Revogar"
            color="error"
            @click="revoke(token)"
          />
        </template>
      </v-list-item>
    </v-list>
    <div v-else-if="!loading" class="text-body-1">Nenhum token.</div>

    <v-dialog v-model="formOpen" max-width="560">
      <v-card>
        <v-card-title class="px-6 pt-5 font-weight-bold">Novo token</v-card-title>
        <v-card-text class="px-6">
          <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">{{
            error
          }}</v-alert>
          <v-form ref="formRef" @submit.prevent="create">
            <v-text-field
              v-model="form.name"
              label="Nome *"
              placeholder="Ex.: notebook da Ana"
              variant="outlined"
              :rules="[requiredRule('Dê um nome ao token')]"
              class="mb-2"
            ></v-text-field>
            <v-combobox
              v-model="form.repositories"
              :items="repositoryChoices"
              label="Repositórios *"
              hint="O nome exato, namespace/* ou * para todos"
              persistent-hint
              variant="outlined"
              multiple
              chips
              closable-chips
              :rules="[(value: string[]) => value.length > 0 || 'Escolha ao menos um']"
              class="mb-2"
            ></v-combobox>
            <div class="text-subtitle-2 font-weight-bold mt-2">O que o token pode</div>
            <div class="d-flex ga-4">
              <v-checkbox
                v-model="form.actions"
                value="pull"
                label="Baixar (pull)"
                color="primary"
                hide-details
              ></v-checkbox>
              <v-checkbox
                v-model="form.actions"
                value="push"
                label="Enviar (push)"
                color="primary"
                hide-details
              ></v-checkbox>
              <v-checkbox
                v-model="form.actions"
                value="delete"
                label="Apagar"
                color="primary"
                hide-details
              ></v-checkbox>
            </div>
            <div class="text-body-2 mb-3">
              Vale o que o seu papel em Servidores permite: leitura só baixa; operador também envia.
            </div>
            <v-select
              v-model="form.days"
              :items="DAYS"
              item-title="title"
              item-value="value"
              label="Validade"
              variant="outlined"
            ></v-select>
          </v-form>
        </v-card-text>
        <v-card-actions class="px-6 pb-4">
          <v-spacer></v-spacer>
          <v-btn variant="text" color="primary" @click="formOpen = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            variant="flat"
            :loading="saving"
            :disabled="!form.actions.length"
            @click="create"
          >
            Criar token
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import CopyableCommand from '@/components/CopyableCommand.vue'
import IconAction from '@/components/IconAction.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { RegistryAction } from '@/bindings/RegistryAction'
import type { RegistryTokenCreated } from '@/bindings/RegistryTokenCreated'
import type { RegistryTokenView } from '@/bindings/RegistryTokenView'
import { useRegistryStore } from '@/stores/registry'
import { requiredRule } from '@/utils/formRules'
import { formatDate, formatRelativeTime } from '@/utils/formatters'
import { ACTION_LABEL, dockerLogin } from '@/utils/registryPresentation'

defineProps<{ host: string }>()

const registry = useRegistryStore()
const { confirm } = useConfirm()
const items = ref<RegistryTokenView[]>([])
const loading = ref(false)
const saving = ref(false)
const error = ref<string | null>(null)
const created = ref<RegistryTokenCreated | null>(null)
const formOpen = ref(false)
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const form = reactive<{
  name: string
  repositories: string[]
  actions: RegistryAction[]
  days: number
}>({
  name: '',
  repositories: [],
  actions: ['pull'],
  days: 90,
})

const DAYS = [
  { title: '30 dias', value: 30 },
  { title: '90 dias', value: 90 },
  { title: '6 meses', value: 180 },
  { title: '1 ano', value: 365 },
]

/** Os repositórios que existem, os namespaces e "todos". */
const repositoryChoices = computed(() => {
  const names = (registry.overview?.repositories ?? []).map((repository) => repository.name)
  const namespaces = [...new Set(names.map((name) => `${name.split('/')[0]}/*`))]
  return ['*', ...namespaces, ...names]
})

function subtitle(token: RegistryTokenView): string {
  const actions = token.scope.actions.map((action) => ACTION_LABEL[action] ?? action).join(', ')
  const used = token.lastUsedAt ? `usado ${formatRelativeTime(token.lastUsedAt)}` : 'nunca usado'
  return [
    token.owner,
    `${actions} em ${token.scope.repositories.join(', ')}`,
    `vence em ${formatDate(token.expiresAt)}`,
    used,
  ].join(' · ')
}

async function load() {
  loading.value = true
  try {
    items.value = await registry.tokens()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível ler os tokens'
  } finally {
    loading.value = false
  }
}

function openForm() {
  form.name = ''
  form.repositories = []
  form.actions = ['pull']
  form.days = 90
  error.value = null
  formOpen.value = true
}

async function create() {
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  saving.value = true
  error.value = null
  try {
    created.value = await registry.createToken({
      name: form.name,
      scope: { repositories: form.repositories, actions: form.actions },
      days: form.days,
    })
    formOpen.value = false
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível criar o token'
  } finally {
    saving.value = false
  }
}

async function revoke(token: RegistryTokenView) {
  const ok = await confirm({
    title: `Revogar "${token.name}"?`,
    message: 'O docker login com este token deixa de funcionar na hora.',
    confirmText: 'Revogar',
    confirmColor: 'error',
    icon: 'mdi-cancel',
  })
  if (!ok) return
  try {
    await registry.revokeToken(token.id)
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível revogar'
  }
}

onMounted(load)
</script>
