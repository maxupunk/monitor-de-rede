<template>
  <v-app>
    <v-navigation-drawer v-model="drawer" permanent>
      <v-list-item title="Monitor de Rede" subtitle="Plataforma"></v-list-item>
      <v-divider></v-divider>
      <v-list density="compact" nav>
        <v-list-item prepend-icon="mdi-view-dashboard" title="Dashboard" to="/"></v-list-item>
        <v-list-item prepend-icon="mdi-domain" title="Sites" to="/sites"></v-list-item>
        <v-list-item prepend-icon="mdi-lan" title="Redes" to="/networks"></v-list-item>
        <v-list-item prepend-icon="mdi-devices" title="Dispositivos" to="/devices"></v-list-item>
        <v-list-item prepend-icon="mdi-sitemap" title="Topologia" to="/topology"></v-list-item>
        <v-list-item prepend-icon="mdi-radar" title="Descoberta" to="/discovery"></v-list-item>
        <v-list-item prepend-icon="mdi-heart-pulse" title="Monitores" to="/monitors"></v-list-item>
        <v-list-item prepend-icon="mdi-bell-outline" title="Alertas" to="/alerts"></v-list-item>
        <v-list-item prepend-icon="mdi-history" title="Eventos" to="/events"></v-list-item>
        <v-list-item prepend-icon="mdi-router-wireless" title="Probes" to="/probes"></v-list-item>
        <v-list-item prepend-icon="mdi-cog" title="Configurações" to="/settings"></v-list-item>
      </v-list>
    </v-navigation-drawer>

    <v-app-bar flat border>
      <v-app-bar-nav-icon @click="drawer = !drawer"></v-app-bar-nav-icon>
      <v-toolbar-title>Sistema de Monitoramento de Redes</v-toolbar-title>
    </v-app-bar>

    <v-main>
      <v-container fluid>
        <router-view></router-view>
      </v-container>
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const drawer = ref(true)
</script>
