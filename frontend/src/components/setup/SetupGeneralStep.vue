<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Notificações & Parâmetros de Monitoramento</div>
      <div class="text-caption text-medium-emphasis">
        Configure os alertas do navegador e as preferências padrão de coleta do sistema.
      </div>
    </div>

    <SetupPushCard class="mb-4" />

    <!-- Parâmetros de Coleta -->
    <v-sheet border rounded="lg" class="pa-4 bg-surface">
      <div class="font-weight-bold text-subtitle-2 mb-3">Parâmetros Padrão de Coleta</div>
      <v-row dense>
        <v-col cols="12" sm="6">
          <v-text-field
            v-model.number="model.defaultPingIntervalSeconds"
            label="Intervalo de Ping Padrão"
            type="number"
            suffix="segundos"
            variant="outlined"
            density="comfortable"
            hint="Frequência de teste para novos monitores (mínimo 10s)"
            persistent-hint
            class="mb-2"
          ></v-text-field>
        </v-col>

        <v-col cols="12" sm="6">
          <v-text-field
            v-model="model.defaultSnmpCommunity"
            label="Comunidade SNMP Padrão"
            variant="outlined"
            density="comfortable"
            hint="Usada na descoberta automática (ex: public)"
            persistent-hint
            class="mb-2"
          ></v-text-field>
        </v-col>

        <v-col cols="12">
          <v-switch
            v-model="model.autoDiscoveryEnabled"
            color="primary"
            label="Varredura automática global periódica das redes ativada"
            density="compact"
            hide-details
          ></v-switch>
        </v-col>
      </v-row>
    </v-sheet>
  </div>
</template>

<script setup lang="ts">
import type { Preferences } from '@/stores/preferences'
import SetupPushCard from './SetupPushCard.vue'

/** Caminho Rede: notificações do navegador e os padrões de coleta. */
const model = defineModel<Preferences>({ required: true })
</script>
