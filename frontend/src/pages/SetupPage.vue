<template>
  <v-app class="bg-grey-darken-4">
    <v-container class="fill-height justify-center align-center" fluid>
      <v-card class="pa-6 pa-md-8 elevation-12 rounded-xl mx-4" max-width="520" width="100%">
        <div class="text-center mb-6">
          <v-avatar color="primary" size="72" class="mb-4 elevation-4">
            <v-icon size="40" color="white">mdi-account-key-outline</v-icon>
          </v-avatar>
          <h1 class="text-h5 font-weight-bold mb-1">Primeiro acesso</h1>
          <p class="text-body-2 text-medium-emphasis">
            Nenhum usuário cadastrado ainda. Crie o administrador do NetMonitor para começar.
          </p>
        </div>

        <v-alert
          type="info"
          variant="tonal"
          density="comfortable"
          class="mb-4 rounded-lg text-body-2"
          icon="mdi-shield-key-outline"
        >
          O token de instalação está no log de inicialização do servidor (procure por
          <code>setup_token</code>) ou na variável <code>SETUP_TOKEN</code>. Também pode ser lido
          com <code>backend_rust-cli task auth_setup_token</code>.
        </v-alert>

        <v-alert
          v-if="authStore.error"
          type="error"
          variant="tonal"
          class="mb-4 rounded-lg"
          closable
          @click:close="authStore.clearError()"
        >
          {{ authStore.error }}
        </v-alert>

        <v-form ref="formRef" validate-on="submit" @submit.prevent="handleSetup">
          <v-text-field
            v-model="name"
            label="Nome"
            autocomplete="name"
            prepend-inner-icon="mdi-account-outline"
            variant="outlined"
            density="comfortable"
            class="mb-2"
            :rules="nameRules"
          ></v-text-field>

          <v-text-field
            v-model="email"
            label="E-mail"
            type="email"
            autocomplete="username"
            prepend-inner-icon="mdi-email-outline"
            variant="outlined"
            density="comfortable"
            class="mb-2"
            :rules="emailRules"
          ></v-text-field>

          <v-text-field
            v-model="password"
            label="Senha"
            :type="showPassword ? 'text' : 'password'"
            autocomplete="new-password"
            prepend-inner-icon="mdi-lock-outline"
            :append-inner-icon="showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
            variant="outlined"
            density="comfortable"
            class="mb-2"
            hint="Mínimo de 8 caracteres"
            :rules="passwordRules"
            @click:append-inner="showPassword = !showPassword"
          ></v-text-field>

          <v-text-field
            v-model="passwordConfirmation"
            label="Confirme a senha"
            :type="showPassword ? 'text' : 'password'"
            autocomplete="new-password"
            prepend-inner-icon="mdi-lock-check-outline"
            variant="outlined"
            density="comfortable"
            class="mb-2"
            :rules="passwordConfirmationRules"
          ></v-text-field>

          <v-text-field
            v-model="token"
            label="Token de instalação"
            autocomplete="off"
            prepend-inner-icon="mdi-key-variant"
            variant="outlined"
            density="comfortable"
            class="mb-4"
            :rules="tokenRules"
          ></v-text-field>

          <v-btn
            type="submit"
            color="primary"
            block
            size="large"
            elevation="2"
            class="text-none font-weight-bold rounded-lg"
            :loading="authStore.loading"
          >
            Criar administrador e entrar
          </v-btn>
        </v-form>
      </v-card>
    </v-container>
  </v-app>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

type ValidationRule = (value: string) => true | string
type VuetifyForm = { validate: () => Promise<{ valid: boolean }> }

const authStore = useAuthStore()
const router = useRouter()

const formRef = ref<VuetifyForm | null>(null)
const name = ref('')
const email = ref('')
const password = ref('')
const passwordConfirmation = ref('')
const token = ref('')
const showPassword = ref(false)

// Espelham a validação do `SetupParams` no backend: reprovar aqui evita a
// viagem até o servidor, mas quem manda continua sendo ele.
const nameRules: ValidationRule[] = [
  (value) => value.trim().length >= 2 || 'O nome precisa ter ao menos 2 caracteres.',
]
const emailRules: ValidationRule[] = [
  (value) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim()) || 'Informe um e-mail válido.',
]
const passwordRules: ValidationRule[] = [
  (value) => value.length >= 8 || 'A senha precisa ter ao menos 8 caracteres.',
]
const passwordConfirmationRules: ValidationRule[] = [
  (value) => value === password.value || 'As senhas não conferem.',
]
const tokenRules: ValidationRule[] = [
  (value) => value.trim().length > 0 || 'Informe o token de instalação.',
]

async function handleSetup() {
  const { valid } = (await formRef.value?.validate()) ?? { valid: false }
  if (!valid) return

  const created = await authStore.completeSetup({
    name: name.value.trim(),
    email: email.value.trim(),
    password: password.value,
    token: token.value.trim(),
  })

  if (created) await router.push({ name: 'dashboard' })
}
</script>
