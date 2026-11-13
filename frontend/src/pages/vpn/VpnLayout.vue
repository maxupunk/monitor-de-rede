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
import { useRoute } from 'vue-router'
import { useVpnStore } from '@/stores/vpn'
import { useAiScreenDetail } from '@/composables/useAiScreenDetail'
import { countNoun } from '@/utils/formatters'

/** A VPN é uma entrada só no menu; servidor e dispositivos são abas dela. */
const TABS = [
  { title: 'Servidor', icon: 'mdi-server-security', name: 'vpn-server' },
  { title: 'Dispositivos', icon: 'mdi-lan-connect', name: 'vpn-devices' },
]

const route = useRoute()
const vpnStore = useVpnStore()

/** O chat sabe a aba aberta e o estado do túnel. */
useAiScreenDetail(() => {
  const tab = TABS.find((item) => item.name === route.name)
  const state = vpnStore.state
  const parts = [
    tab ? `aba ${tab.title}` : null,
    state && !state.configured ? 'servidor VPN não configurado' : null,
    state?.configured
      ? `${state.peersConnected} de ${countNoun(state.peersTotal, 'peer')} conectados`
      : null,
    state?.cidr ? `faixa ${state.cidr}` : null,
  ]
  return parts.filter(Boolean).join(', ')
})
</script>
