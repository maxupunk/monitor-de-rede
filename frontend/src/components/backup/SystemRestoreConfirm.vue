<template>
  <v-alert type="warning" variant="tonal" border="start" icon="mdi-backup-restore">
    <div class="font-weight-bold mb-1">Voltar o NetMonitor para esta cópia?</div>
    <div class="text-body-2 mb-3">
      Tudo o que está configurado agora — dispositivos, monitores, alertas, VPN e preferências — é
      substituído pelo que está na cópia, e o histórico de coleta dos equipamentos atuais é
      descartado. Usuários e senhas não mudam; a página recarrega ao terminar.
    </div>
    <BackupCountsSummary :name="name" :counts="counts" class="mb-3" />
    <div class="d-flex flex-wrap ga-2">
      <v-btn
        color="error"
        variant="flat"
        prepend-icon="mdi-backup-restore"
        :loading="loading"
        @click="emit('confirm')"
      >
        Restaurar esta cópia
      </v-btn>
      <v-btn variant="text" color="primary" :disabled="loading" @click="emit('cancel')">
        Cancelar
      </v-btn>
    </div>
  </v-alert>
</template>

<script setup lang="ts">
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import BackupCountsSummary from './BackupCountsSummary.vue'

/**
 * O passo entre "escolhi a cópia" e "apaguei a configuração atual": o que vai
 * mudar e o que a cópia contém, antes do clique que não tem volta.
 */
defineProps<{
  name: string
  counts: BackupCountsResponse
  loading: boolean
}>()

const emit = defineEmits<{ confirm: []; cancel: [] }>()
</script>
