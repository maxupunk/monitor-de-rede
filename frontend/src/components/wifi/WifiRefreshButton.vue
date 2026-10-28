<template>
  <v-tooltip :text="READ_ONLY" :disabled="auth.canWrite" location="bottom">
    <template #activator="{ props: tip }">
      <span v-bind="tip">
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-refresh"
          :loading="store.requesting || reading"
          :disabled="!auth.canWrite"
          @click="emit('refresh')"
        >
          Ler agora
        </v-btn>
      </span>
    </template>
  </v-tooltip>
</template>

<script setup lang="ts">
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'

defineProps<{ reading: boolean }>()
const emit = defineEmits<{ (e: 'refresh'): void }>()

const READ_ONLY = 'Seu perfil só consulta: a leitura roda um comando no roteador.'

const auth = useAuthStore()
const store = useWifiClientsStore()
</script>
