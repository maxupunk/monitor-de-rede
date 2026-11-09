<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 640"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-key-chain</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Credenciais do GitHub</v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          Um token lê o repositório (o compose) e baixa as imagens do ghcr.io. Fica cifrado aqui e
          só vai ao servidor durante o deploy — nunca fica salvo lá.
        </v-card-subtitle>
      </v-card-item>

      <v-card-text class="px-6">
        <v-alert
          v-if="error"
          type="error"
          variant="tonal"
          closable
          class="mb-4"
          @click:close="error = null"
        >
          {{ error }}
        </v-alert>

        <template v-if="!editing">
          <v-list
            v-if="deploys.credentials.length"
            density="compact"
            class="bg-transparent pa-0 mb-2"
          >
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
          <div v-else class="text-body-2 text-high-emphasis mb-4">
            Nenhuma credencial. Repositórios e imagens públicos funcionam sem.
          </div>
          <v-btn color="primary" variant="tonal" prepend-icon="mdi-plus" @click="edit(null)">
            Nova credencial
          </v-btn>
        </template>

        <v-form v-else ref="formRef" autocomplete="off" @submit.prevent="save">
          <v-text-field
            v-model="form.name"
            label="Nome *"
            placeholder="Ex.: GitHub Master Sistemas"
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
            Crie um token <strong>clássico</strong> (o ghcr.io não aceita o fine-grained) com
            <code>read:packages</code> e, se o repositório for privado, <code>repo</code>. Em GitHub
            → Settings → Developer settings → Personal access tokens → Tokens (classic).
          </v-alert>
          <v-text-field
            v-model="form.registry"
            label="Registry"
            hint="Vazio usa o ghcr.io"
            persistent-hint
            variant="outlined"
          ></v-text-field>
        </v-form>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <template v-if="editing">
          <v-btn variant="text" @click="editing = false">Voltar</v-btn>
          <v-spacer></v-spacer>
          <v-btn color="primary" variant="flat" :loading="saving" @click="save">Salvar</v-btn>
        </template>
        <template v-else>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="emit('update:modelValue', false)">Fechar</v-btn>
        </template>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import IconAction from '@/components/IconAction.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DeployCredentialView } from '@/bindings/DeployCredentialView'
import { useDeploysStore } from '@/stores/deploys'
import { requiredRule } from '@/utils/formRules'

defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

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
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar a credencial'
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
