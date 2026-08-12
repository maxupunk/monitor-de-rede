<template>
  <v-app class="bg-grey-darken-4">
    <v-container class="fill-height justify-center align-center" fluid>
      <v-card class="pa-6 pa-md-8 elevation-12 rounded-xl mx-4" max-width="440" width="100%">
        <div class="text-center mb-6">
          <v-avatar color="primary" size="72" class="mb-4 elevation-4">
            <v-icon size="40" color="white">mdi-shield-network-outline</v-icon>
          </v-avatar>
          <h1 class="text-h4 font-weight-bold mb-1">NetMonitor</h1>
          <p class="text-subtitle-2 text-medium-emphasis">Plataforma de Monitoramento de Redes</p>
        </div>

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

        <v-form ref="formRef" validate-on="submit" @submit.prevent="handleLogin">
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
            autocomplete="current-password"
            prepend-inner-icon="mdi-lock-outline"
            :append-inner-icon="showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
            variant="outlined"
            density="comfortable"
            class="mb-4"
            :rules="passwordRules"
            @click:append-inner="showPassword = !showPassword"
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
            Entrar no Sistema
          </v-btn>
        </v-form>
      </v-card>
    </v-container>
  </v-app>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

type ValidationRule = (value: string) => true | string
type VuetifyForm = { validate: () => Promise<{ valid: boolean }> }

const authStore = useAuthStore()
const router = useRouter()
const route = useRoute()

const formRef = ref<VuetifyForm | null>(null)
const email = ref('')
const password = ref('')
const showPassword = ref(false)

const emailRules: ValidationRule[] = [
  (value) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim()) || 'Informe um e-mail válido.',
]
const passwordRules: ValidationRule[] = [(value) => value.length > 0 || 'Informe a senha.']

/**
 * Volta para onde o guard interrompeu a navegação.
 *
 * Só aceita caminho relativo: um `?redirect=https://…` vindo de um link
 * plantado transformaria o login numa rampa de redirecionamento para fora do
 * sistema, com a credencial recém-digitada ainda fresca na aba.
 */
function redirectTarget(): string {
  const requested = route.query.redirect
  if (typeof requested === 'string' && requested.startsWith('/') && !requested.startsWith('//')) {
    return requested
  }
  return '/'
}

async function handleLogin() {
  const { valid } = (await formRef.value?.validate()) ?? { valid: false }
  if (!valid) return

  const success = await authStore.login(email.value.trim(), password.value)
  if (success) await router.push(redirectTarget())
}
</script>
