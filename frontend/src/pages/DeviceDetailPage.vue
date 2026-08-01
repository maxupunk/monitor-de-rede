<template>
  <div>
    <h1 class="text-h5 font-weight-bold mb-4">Detalhes do Dispositivo</h1>
    <v-card flat border>
      <v-tabs v-model="tab">
        <v-tab value="overview">Visão geral</v-tab>
        <v-tab value="monitors">Monitoramento</v-tab>
        <v-tab value="interfaces">Interfaces</v-tab>
        <v-tab value="traffic">Tráfego</v-tab>
        <v-tab value="services">Serviços</v-tab>
        <v-tab value="topology">Topologia</v-tab>
        <v-tab value="snmp">SNMP</v-tab>
        <v-tab value="events">Eventos</v-tab>
        <v-tab value="settings">Configurações</v-tab>
      </v-tabs>
      <v-card-text>
        <v-window v-model="tab">
          <v-window-item value="overview">Informações gerais do dispositivo</v-window-item>
        </v-window>
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const tab = ref('overview')
</script>
