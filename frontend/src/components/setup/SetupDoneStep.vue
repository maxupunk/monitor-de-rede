<template>
  <div class="text-center py-6">
    <v-avatar color="success" size="72" class="mb-4 elevation-2">
      <v-icon size="42" color="white">mdi-check-bold</v-icon>
    </v-avatar>
    <h3 class="text-h5 font-weight-bold mb-2">Tudo pronto!</h3>
    <p class="text-body-1 max-w-600 mx-auto mb-6">
      As configurações básicas foram salvas com sucesso no servidor. O NetMonitor já está ativo e
      operando.
    </p>

    <div class="d-flex justify-center flex-wrap ga-3">
      <v-btn
        v-for="action in actions"
        :key="action.label"
        :color="action.color"
        size="large"
        :variant="action.variant"
        :prepend-icon="action.icon"
        class="font-weight-bold"
        @click="emit('navigate', action.to)"
      >
        {{ action.label }}
      </v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

interface DoneAction {
  label: string
  icon: string
  color: string
  variant: 'flat' | 'tonal' | 'outlined'
  to: RouteLocationRaw
}

const props = defineProps<{
  /** Caminho Rede: cadastro de dispositivo e, com varredura imediata, a descoberta. */
  network: boolean
  scanStarted: boolean
  /** Caminho Servidores: o Docker dos servidores conectados. */
  servers: boolean
}>()

const emit = defineEmits<{ navigate: [to: RouteLocationRaw] }>()

const actions = computed<DoneAction[]>(() => {
  const list: DoneAction[] = [
    {
      label: 'Ir para o Dashboard',
      icon: 'mdi-view-dashboard',
      color: 'primary',
      variant: 'flat',
      to: '/',
    },
  ]
  if (props.network && props.scanStarted) {
    list.push({
      label: 'Acompanhar Descoberta',
      icon: 'mdi-radar',
      color: 'secondary',
      variant: 'tonal',
      to: '/discovery',
    })
  }
  if (props.network) {
    list.push({
      label: 'Cadastrar Dispositivo',
      icon: 'mdi-plus',
      color: 'info',
      variant: 'outlined',
      to: '/devices',
    })
  }
  if (props.servers) {
    list.push({
      label: 'Abrir Docker',
      icon: 'mdi-docker',
      color: 'info',
      variant: 'tonal',
      to: '/docker',
    })
  }
  if (!props.network) {
    list.push({
      label: 'Canais de notificação',
      icon: 'mdi-bell-cog-outline',
      color: 'warning',
      variant: 'tonal',
      to: { path: '/settings', query: { tab: 'notificacoes' } },
    })
  }
  return list
})
</script>

<style scoped>
.max-w-600 {
  max-width: 600px;
}
</style>
