<template>
  <v-card elevation="2" class="rounded-lg pa-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="warning">mdi-bell-ring-outline</v-icon>
      Notificações PWA do Navegador
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-caption text-grey-darken-1 mb-3">
        Configure as notificações do sistema PWA para receber alertas de queda e falhas críticas
        diretamente na área de trabalho ou dispositivo móvel.
      </p>

      <div class="d-flex align-center justify-space-between mb-4 pa-3 rounded border">
        <div>
          <div class="font-weight-bold text-subtitle-2">Status da Permissão</div>
          <div class="text-caption text-grey">
            {{
              permissionState === 'granted'
                ? 'Permissão concedida no navegador'
                : permissionState === 'denied'
                  ? 'Bloqueado no navegador'
                  : permissionState === 'unsupported'
                    ? 'Navegador não suporta notificações'
                    : 'Ainda não solicitado'
            }}
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
          {{ permissionState.toUpperCase() }}
        </v-chip>
      </div>

      <v-switch
        :model-value="notificationsEnabled"
        label="Ativar Notificações Nativas em Tempo Real"
        color="primary"
        :disabled="permissionState !== 'granted'"
        @update:model-value="(val) => setNotificationsEnabled(Boolean(val))"
      />
    </v-card-text>
    <v-card-actions class="justify-space-between flex-wrap ga-2 pt-0">
      <v-btn
        v-if="permissionState !== 'granted'"
        color="primary"
        variant="flat"
        size="small"
        prepend-icon="mdi-bell-check"
        :disabled="permissionState === 'unsupported'"
        @click="requestPermission"
      >
        Solicitar Permissão
      </v-btn>
      <v-btn
        v-else
        color="secondary"
        variant="tonal"
        size="small"
        prepend-icon="mdi-bell-ring"
        @click="emit('test-notification')"
      >
        Enviar Notificação de Teste
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { useNotifications } from '@/composables/useNotifications'

const emit = defineEmits<{
  'test-notification': []
}>()

const { permissionState, notificationsEnabled, requestPermission, setNotificationsEnabled } =
  useNotifications()
</script>
