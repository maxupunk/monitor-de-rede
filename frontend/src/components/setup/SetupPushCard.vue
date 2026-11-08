<template>
  <v-sheet border rounded="lg" class="pa-4 bg-surface">
    <div class="d-flex align-start justify-space-between flex-wrap ga-3 mb-3">
      <div class="d-flex align-center ga-3">
        <v-avatar color="warning" size="40" rounded="lg" variant="tonal">
          <v-icon size="22">mdi-bell-ring-outline</v-icon>
        </v-avatar>
        <div>
          <div class="font-weight-bold text-subtitle-1">Notificações PWA do Navegador</div>
          <div class="text-caption text-high-emphasis">
            Receba alertas imediatos na área de trabalho quando dispositivos caírem.
          </div>
        </div>
      </div>
      <v-chip
        :color="
          permissionState === 'granted'
            ? 'success'
            : permissionState === 'denied'
              ? 'error'
              : 'warning'
        "
        size="small"
        variant="tonal"
        class="font-weight-bold"
      >
        {{
          permissionState === 'granted'
            ? 'PERMITIDO'
            : permissionState === 'denied'
              ? 'BLOQUEADO'
              : 'NÃO SOLICITADO'
        }}
      </v-chip>
    </div>

    <div class="d-flex align-center ga-2 flex-wrap">
      <v-btn
        v-if="permissionState !== 'granted'"
        color="primary"
        variant="flat"
        size="small"
        prepend-icon="mdi-bell-check"
        @click="requestPermission"
      >
        Ativar Notificações no Navegador
      </v-btn>
      <v-btn
        v-else
        color="success"
        variant="tonal"
        size="small"
        prepend-icon="mdi-bell-ring"
        @click="testNotification"
      >
        Enviar Notificação de Teste
      </v-btn>
    </div>
  </v-sheet>
</template>

<script setup lang="ts">
import { useNotifications } from '@/composables/useNotifications'

/** Alertas do navegador (PWA): pedir a permissão e testar, na hora. */
const { permissionState, requestPermission, sendNotification } = useNotifications()

function testNotification() {
  sendNotification('NetMonitor - Notificação de Teste', {
    body: 'As notificações PWA em tempo real estão configuradas e operacionais!',
  })
}
</script>
