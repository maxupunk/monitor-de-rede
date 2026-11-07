<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="primary">mdi-rocket-launch-outline</v-icon>
      Assistente de Configuração Inicial
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-body-2 mb-3">
        Execute novamente o assistente de primeiro acesso para escolher o que a instalação gerencia
        (Rede, Servidores e Docker, VPN) e configurar, de forma guiada, as etapas dessas áreas:
        endereços deste servidor, sites, sub-redes, DNS, agentes e notificações.
      </p>
    </v-card-text>
    <v-card-actions class="justify-end">
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-auto-fix"
        @click="onboardingStore.openWizard()"
      >
        Abrir Assistente de Configuração
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { useOnboardingStore } from '@/stores/onboarding'

const onboardingStore = useOnboardingStore()
</script>
