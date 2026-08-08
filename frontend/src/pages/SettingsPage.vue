<template>
  <div>
    <PageHeader
      title="Configurações do Sistema"
      subtitle="Preferências globais, parâmetros de monitoramento e notificações"
    />

    <v-row>
      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg pa-4">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="primary">mdi-cog-outline</v-icon>
            Geral & Monitoramento
          </v-card-title>
          <v-card-text class="mt-2">
            <v-text-field
              v-model="defaultPingInterval"
              label="Intervalo Padrão de Ping (segundos)"
              type="number"
              variant="outlined"
            />
            <v-text-field
              v-model="defaultSnmpCommunity"
              label="Comunidade SNMP Padrão"
              variant="outlined"
            />
            <v-switch
              v-model="autoDiscovery"
              label="Habilitar Descoberta Automática Periódica"
              color="primary"
            />
          </v-card-text>
          <v-card-actions class="justify-end">
            <v-btn color="primary" @click="saveSettings">Salvar Preferências</v-btn>
          </v-card-actions>
        </v-card>
      </v-col>

      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg pa-4">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="primary">mdi-cellphone-link</v-icon>
            Recursos PWA & Dispositivo
          </v-card-title>
          <v-card-text class="mt-2">
            <p class="text-body-2 text-grey-darken-1 mb-4">
              Esta plataforma suporta PWA (Progressive Web App). Você pode instalá-la como um
              aplicativo nativo no seu computador ou dispositivo móvel para receber notificações e
              acesso offline rápido.
            </p>
            <v-chip color="success" size="small" variant="tonal" class="mb-4">
              Service Worker PWA Registrado
            </v-chip>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'

const defaultPingInterval = ref(60)
const defaultSnmpCommunity = ref('public')
const autoDiscovery = ref(true)

function saveSettings() {
  alert('Configurações salvas com sucesso!')
}
</script>
