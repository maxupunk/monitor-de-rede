<template>
  <div>
    <v-alert
      v-if="error"
      type="error"
      variant="tonal"
      density="compact"
      closable
      class="mb-3"
      @click:close="error = null"
    >
      {{ error }}
    </v-alert>

    <template v-if="!editing">
      <v-list v-if="deploys.credentials.length" density="compact" class="bg-transparent pa-0 mb-2">
        <v-list-item
          v-for="credential in deploys.credentials"
          :key="credential.id"
          :title="credential.name"
          :subtitle="`${credential.username} @ ${credential.registry}`"
          class="px-0"
        >
          <template #prepend>
            <v-icon color="primary" class="mr-3">mdi-key</v-icon>
          </template>
          <template #append>
            <IconAction
              icon="mdi-pencil"
              label="Editar"
              color="primary"
              @click="edit(credential)"
            />
            <IconAction
              icon="mdi-delete"
              label="Excluir"
              color="error"
              @click="remove(credential)"
            />
          </template>
        </v-list-item>
      </v-list>
      <div v-else class="text-body-2 text-high-emphasis mb-3">
        Nenhum token. Imagens públicas no ghcr.io não precisam.
      </div>
      <v-btn color="primary" variant="tonal" prepend-icon="mdi-plus" @click="edit(null)">
        Novo token
      </v-btn>
    </template>

    <v-form v-else ref="formRef" autocomplete="off" @submit.prevent="save">
      <v-text-field
        v-model="form.name"
        label="Nome *"
        placeholder="Ex.: Imagens da Master Sistemas"
        variant="outlined"
        :rules="[requiredRule('Informe um nome')]"
        class="mb-2"
      ></v-text-field>
      <v-text-field
        v-model="form.username"
        label="Usuário do GitHub *"
        hint="O login de quem gerou o token"
        persistent-hint
        variant="outlined"
        :rules="[requiredRule('Informe o usuário')]"
        class="mb-2"
      ></v-text-field>
      <v-text-field
        v-model="form.secret"
        :label="editingId ? 'Token (vazio mantém o salvo)' : 'Token *'"
        :type="showSecret ? 'text' : 'password'"
        :append-inner-icon="showSecret ? 'mdi-eye-off' : 'mdi-eye'"
        variant="outlined"
        :rules="editingId ? [] : [requiredRule('Informe o token')]"
        class="mb-2"
        @click:append-inner="showSecret = !showSecret"
      ></v-text-field>
      <v-alert type="info" variant="tonal" density="compact" class="mb-4">
        Crie um token <strong>clássico</strong> com <code>read:packages</code> (o ghcr.io não aceita
        o fine-grained nem o do GitHub App). Sem conexão com o GitHub, acrescente
        <code>repo</code> para ler repositório privado.
        <a
          href="https://github.com/settings/tokens/new?scopes=read:packages&description=NetMonitor%20deploy"
          target="_blank"
          rel="noopener"
          class="font-weight-bold"
          >Criar o token no GitHub</a
        >
      </v-alert>
      <v-text-field
        v-model="form.registry"
        label="Registry"
        hint="Vazio usa o ghcr.io"
        persistent-hint
        variant="outlined"
      ></v-text-field>
      <div class="d-flex ga-2 mt-4">
        <v-btn variant="text" @click="editing = false">Voltar</v-btn>
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">Salvar</v-btn>
      </div>
    </v-form>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import IconAction from '@/components/IconAction.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DeployCredentialView } from '@/bindings/DeployCredentialView'
import { useDeploysStore } from '@/stores/deploys'
import { requiredRule } from '@/utils/formRules'

const deploys = useDeploysStore()
const { confirm } = useConfirm()

const editing = ref(false)
const editingId = ref<number | null>(null)
const saving = ref(false)
const error = ref<string | null>(null)
const showSecret = ref(false)
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const form = reactive({ name: '', username: '', secret: '', registry: '' })

function edit(credential: DeployCredentialView | null) {
  editingId.value = credential?.id ?? null
  form.name = credential?.name ?? ''
  form.username = credential?.username ?? ''
  form.registry = credential && credential.registry !== 'ghcr.io' ? credential.registry : ''
  form.secret = ''
  showSecret.value = false
  error.value = null
  editing.value = true
}

async function save() {
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  saving.value = true
  error.value = null
  try {
    await deploys.saveCredential(editingId.value, {
      name: form.name,
      username: form.username,
      secret: form.secret || undefined,
      registry: form.registry || undefined,
    })
    editing.value = false
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar o token'
  } finally {
    saving.value = false
  }
}

async function remove(credential: DeployCredentialView) {
  const ok = await confirm({
    title: `Excluir "${credential.name}"?`,
    message: 'O token é apagado desta central.',
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  try {
    await deploys.removeCredential(credential.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível excluir'
  }
}
</script>
