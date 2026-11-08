<template>
  <div>
    <v-card elevation="2" rounded="lg" class="mb-4 overflow-hidden">
      <v-tabs color="primary" show-arrows>
        <v-tab v-for="item in TABS" :key="item.name" :to="{ name: item.name }" exact>
          <v-icon start>{{ item.icon }}</v-icon>
          {{ item.title }}
        </v-tab>
      </v-tabs>
    </v-card>
    <router-view />
  </div>
</template>

<script setup lang="ts">
/** A VPN é uma entrada só no menu; servidor e dispositivos são abas dela. */
const TABS = [
  { title: 'Servidor', icon: 'mdi-server-security', name: 'vpn-server' },
  { title: 'Dispositivos', icon: 'mdi-lan-connect', name: 'vpn-devices' },
]
</script>
