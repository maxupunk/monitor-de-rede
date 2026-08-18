<template>
  <v-dialog :model-value="modelValue" max-width="640" persistent @update:model-value="close">
    <v-card rounded="xl">
      <v-card-title class="d-flex align-center pa-5 pb-3">
        <v-avatar color="primary" variant="tonal" size="40" class="mr-3">
          <v-icon>{{ user ? 'mdi-account-edit-outline' : 'mdi-account-plus-outline' }}</v-icon>
        </v-avatar>
        <div>
          <div class="text-h6">{{ user ? 'Editar usuário' : 'Novo usuário' }}</div>
          <div class="text-caption text-medium-emphasis">
            {{
              user
                ? 'Atualize os dados e o nível de acesso.'
                : 'Crie uma conta com o menor acesso necessário.'
            }}
          </div>
        </div>
      </v-card-title>

      <v-divider></v-divider>

      <v-card-text class="pa-5">
        <v-alert
          v-if="usersStore.error"
          type="error"
          variant="tonal"
          closable
          class="mb-4"
          @click:close="usersStore.clearError"
        >
          {{ usersStore.error }}
        </v-alert>

        <v-form ref="formRef" @submit.prevent="save">
          <v-row>
            <v-col cols="12" md="6">
              <v-text-field
                v-model="form.name"
                label="Nome completo"
                prepend-inner-icon="mdi-account-outline"
                variant="outlined"
                autocomplete="name"
                :rules="nameRules"
              ></v-text-field>
            </v-col>
            <v-col cols="12" md="6">
              <v-text-field
                v-model="form.email"
                label="E-mail"
                prepend-inner-icon="mdi-email-outline"
                variant="outlined"
                type="email"
                autocomplete="email"
                :rules="emailRules"
              ></v-text-field>
            </v-col>
            <v-col cols="12">
              <v-text-field
                v-model="form.password"
                :label="user ? 'Nova senha (opcional)' : 'Senha temporária'"
                prepend-inner-icon="mdi-lock-outline"
                variant="outlined"
                :type="showPassword ? 'text' : 'password'"
                :append-inner-icon="showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
                :autocomplete="user ? 'new-password' : 'new-password'"
                :hint="
                  user
                    ? 'Deixe em branco para manter a senha atual.'
                    : 'Mínimo de 8 caracteres e uma letra maiúscula.'
                "
                persistent-hint
                :rules="passwordRules"
                @click:append-inner="showPassword = !showPassword"
              ></v-text-field>
            </v-col>
            <v-col cols="12">
              <v-select
                v-model="form.role"
                label="Perfil de acesso"
                prepend-inner-icon="mdi-shield-account-outline"
                variant="outlined"
                :items="ROLE_OPTIONS"
                item-title="title"
                item-value="value"
                :hint="roleHint"
                persistent-hint
                :disabled="isCurrentUser"
              ></v-select>
            </v-col>
            <v-col cols="12">
              <v-switch
                v-model="form.active"
                color="success"
                label="Usuário ativo"
                hint="Contas desativadas não conseguem iniciar nem manter uma sessão."
                persistent-hint
                :disabled="isCurrentUser"
              ></v-switch>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" :disabled="usersStore.saving" @click="close(false)">Cancelar</v-btn>
        <v-btn
          color="primary"
          variant="flat"
          :loading="usersStore.saving"
          prepend-icon="mdi-content-save-outline"
          @click="save"
        >
          {{ user ? 'Salvar alterações' : 'Criar usuário' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useUsersStore, type ManagedUser } from '@/stores/users'
import { ROLE_OPTIONS, type UserRole } from '@/utils/access'

const props = defineProps<{
  modelValue: boolean
  user: ManagedUser | null
  currentUserId?: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: []
}>()

const usersStore = useUsersStore()
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const showPassword = ref(false)
const form = reactive({
  name: '',
  email: '',
  password: '',
  role: 'viewer' as UserRole,
  active: true,
})

const isCurrentUser = computed(() => props.user?.id === props.currentUserId)
const selectedRole = computed(() => ROLE_OPTIONS.find((item) => item.value === form.role))
const roleHint = computed(() =>
  isCurrentUser.value
    ? 'Seu próprio perfil não pode ser alterado.'
    : selectedRole.value?.description
)

const nameRules = [(value: string) => value.trim().length >= 2 || 'Informe ao menos 2 caracteres.']
const emailRules = [
  (value: string) => /.+@.+\..+/.test(value.trim()) || 'Informe um e-mail válido.',
]
const passwordRules = computed(() => [
  (value: string) => Boolean(props.user) || value.length >= 8 || 'Informe ao menos 8 caracteres.',
  (value: string) => !value || value.length >= 8 || 'Informe ao menos 8 caracteres.',
  (value: string) => !value || /[A-Z]/.test(value) || 'Inclua ao menos uma letra maiúscula.',
])

watch(
  () => [props.modelValue, props.user] as const,
  ([open, user]) => {
    if (!open) return
    usersStore.clearError()
    showPassword.value = false
    form.name = user?.fullName ?? ''
    form.email = user?.email ?? ''
    form.password = ''
    form.role = user?.role ?? 'viewer'
    form.active = user?.active ?? true
  },
  { immediate: true }
)

function close(value = false) {
  if (!usersStore.saving) emit('update:modelValue', value)
}

async function save() {
  const result = await formRef.value?.validate()
  if (!result?.valid) return

  const payload = {
    name: form.name.trim(),
    email: form.email.trim(),
    password: form.password,
    role: form.role,
    active: form.active,
  }
  const saved = props.user
    ? await usersStore.updateUser(props.user.id, payload)
    : await usersStore.createUser(payload)
  if (!saved) return
  emit('saved')
  emit('update:modelValue', false)
}
</script>
