<template>
  <v-app class="bg-grey-darken-4">
    <v-container class="fill-height justify-center align-center" fluid>
      <v-card class="pa-6 pa-md-8 elevation-12 rounded-xl mx-4" max-width="440" width="100%">
        <div class="text-center mb-6">
          <v-avatar color="primary" size="72" class="mb-4 elevation-4">
            <v-icon size="40" color="white">mdi-shield-network-outline</v-icon>
          </v-avatar>
          <h1 class="text-h4 font-weight-bold mb-1">NetMonitor</h1>
          <p class="text-subtitle-2 text-grey-darken-1">Plataforma de Monitoramento de Redes</p>
        </div>

        <v-alert
          v-if="authStore.error"
          type="error"
          variant="tonal"
          class="mb-4 rounded-lg"
          closable
        >
          {{ authStore.error }}
        </v-alert>

        <v-form @submit.prevent="handleLogin">
          <v-text-field
            v-model="email"
            label="E-mail"
            type="email"
            prepend-inner-icon="mdi-email-outline"
            variant="outlined"
            density="comfortable"
            class="mb-2"
            required
          />

          <v-text-field
            v-model="password"
            label="Senha"
            type="password"
            prepend-inner-icon="mdi-lock-outline"
            variant="outlined"
            density="comfortable"
            class="mb-4"
            required
          />

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
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const email = ref('admin@monitor.local')
const password = ref('admin123')
const authStore = useAuthStore()
const router = useRouter()

async function handleLogin() {
  const success = await authStore.login(email.value, password.value)
  if (success) {
    router.push('/')
  }
}
</script>
