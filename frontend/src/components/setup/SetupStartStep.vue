<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Como você quer começar?</div>
      <div class="text-body-2">
        Já tem um NetMonitor rodando em outro servidor, ou um backup guardado? Traga tudo de volta
        de uma vez. Senão, o assistente configura esta instalação do zero, passo a passo.
      </div>
    </div>

    <v-row>
      <v-col cols="12" sm="6">
        <ChoiceCard
          color="primary"
          class="pa-4 h-100"
          data-choice="restore"
          @click="emit('restore')"
        >
          <div class="d-flex align-center ga-3 mb-2">
            <v-avatar color="primary" variant="tonal" rounded="lg" size="40">
              <v-icon size="22">mdi-backup-restore</v-icon>
            </v-avatar>
            <span class="font-weight-bold text-subtitle-1">Restaurar um backup</span>
          </div>
          <p class="text-body-2 mb-0">
            Envie o arquivo do <strong>backup completo</strong> (<code>.json.gz</code>) ou da
            configuração (<code>.json</code>) baixado na tela de Backup de outra instalação.
          </p>
        </ChoiceCard>
      </v-col>
      <v-col cols="12" sm="6">
        <ChoiceCard color="success" class="pa-4 h-100" data-choice="fresh" @click="emit('fresh')">
          <div class="d-flex align-center ga-3 mb-2">
            <v-avatar color="success" variant="tonal" rounded="lg" size="40">
              <v-icon size="22">mdi-rocket-launch-outline</v-icon>
            </v-avatar>
            <span class="font-weight-bold text-subtitle-1">Configurar do zero</span>
          </div>
          <p class="text-body-2 mb-0">
            Escolha o que vai acompanhar e cadastre o local, os endereços e a rede em poucas etapas.
          </p>
        </ChoiceCard>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import ChoiceCard from '@/components/ChoiceCard.vue'

/** A escolha antes das etapas: trazer um backup ou configurar do zero. Só o administrador a vê. */
const emit = defineEmits<{ restore: []; fresh: [] }>()
</script>
