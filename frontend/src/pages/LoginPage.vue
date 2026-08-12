<template>
  <AuthShell title="Entrar" subtitle="Informe suas credenciais para acessar o painel.">
    <v-expand-transition>
      <v-alert
        v-if="authStore.error"
        type="error"
        variant="tonal"
        density="comfortable"
        class="mb-5"
        rounded="lg"
        closable
        @click:close="authStore.clearError()"
      >
        {{ authStore.error }}
      </v-alert>
    </v-expand-transition>

    <v-form ref="formRef" validate-on="submit" @submit.prevent="handleLogin">
      <v-text-field
        v-model="email"
        label="E-mail"
        type="email"
        autocomplete="username"
        autofocus
        prepend-inner-icon="mdi-email-outline"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        class="mb-3"
        hide-details="auto"
        :rules="emailRules"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <v-text-field
        v-model="password"
        label="Senha"
        :type="showPassword ? 'text' : 'password'"
        autocomplete="current-password"
        prepend-inner-icon="mdi-lock-outline"
        :append-inner-icon="showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        class="mb-6"
        hide-details="auto"
        :rules="passwordRules"
        @click:append-inner="showPassword = !showPassword"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <v-btn
        type="submit"
        color="primary"
        block
        size="large"
        variant="flat"
        rounded="lg"
        class="text-none font-weight-bold"
        :loading="authStore.loading"
        append-icon="mdi-arrow-right"
      >
        Entrar no sistema
      </v-btn>
    </v-form>
  </AuthShell>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AuthShell from '@/components/auth/AuthShell.vue'
import { useAuthStore } from '@/stores/auth'
import { emailRule, requiredRule, type ValidationRule, type VuetifyForm } from '@/utils/formRules'

const authStore = useAuthStore()
const router = useRouter()
const route = useRoute()

const formRef = ref<VuetifyForm | null>(null)
const email = ref('')
const password = ref('')
const showPassword = ref(false)

const emailRules: ValidationRule[] = [emailRule()]
const passwordRules: ValidationRule[] = [requiredRule('Informe a senha.')]

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
