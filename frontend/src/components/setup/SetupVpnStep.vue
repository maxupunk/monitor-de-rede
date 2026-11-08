<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-3">
      <div>
        <div class="text-h6 font-weight-bold">Servidor WireGuard VPN (Opcional)</div>
        <div class="text-caption text-high-emphasis">
          Permite que roteadores (MikroTik, OpenWrt, Linux) criem túneis seguros para este servidor.
        </div>
      </div>
      <v-switch
        v-model="model.enabled"
        color="primary"
        label="Configurar WireGuard"
        density="compact"
        hide-details
      ></v-switch>
    </div>

    <v-expand-transition>
      <v-sheet v-if="model.enabled" border rounded="lg" class="pa-4 mb-4 bg-surface">
        <v-row dense>
          <v-col cols="12" md="6">
            <v-text-field
              v-model="model.publicEndpoint"
              label="Endpoint Público (IP ou DDNS) *"
              placeholder="Ex: 203.0.113.15 ou vpn.empresa.com"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-web"
              hint="Endereço onde os roteadores remotos se conectarão"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12" sm="6" md="3">
            <v-text-field
              v-model.number="model.listenPort"
              label="Porta UDP *"
              type="number"
              placeholder="51820"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-numeric"
              hint="Porta padrão: 51820"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12" sm="6" md="3">
            <v-text-field
              v-model="model.cidr"
              label="Sub-rede da VPN *"
              placeholder="10.8.0.0/24"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-shield-outline"
              hint="Faixa interna privada"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12" class="mt-2">
            <div class="d-flex align-center justify-space-between flex-wrap ga-2">
              <v-btn
                color="primary"
                variant="tonal"
                size="small"
                prepend-icon="mdi-radar"
                :loading="vpnTesting"
                @click="handleTestVpnPreflight"
              >
                Testar Acessibilidade Externa (Pré-voo UDP)
              </v-btn>
            </div>

            <v-alert
              v-if="vpnPreflightResult"
              :type="vpnPreflightResult.level"
              variant="tonal"
              density="compact"
              class="mt-3"
            >
              <div class="font-weight-bold">{{ vpnPreflightResult.message }}</div>
              <div class="text-caption">{{ vpnPreflightResult.recommendation }}</div>
            </v-alert>
          </v-col>
        </v-row>
      </v-sheet>
    </v-expand-transition>

    <v-card variant="tonal" color="info" class="pa-4 rounded-lg">
      <div class="d-flex align-start ga-3">
        <v-icon color="info" size="24">mdi-help-circle-outline</v-icon>
        <div class="text-caption">
          Não precisa de VPN agora? Deixe desmarcado e ative quando desejar através do menu
          <strong>VPN WireGuard -> Servidor VPN</strong>.
        </div>
      </div>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useVpnStore, type VpnPreflightResult } from '@/stores/vpn'
import type { VpnForm } from './setupForm'

const model = defineModel<VpnForm>({ required: true })

const vpnStore = useVpnStore()
const vpnTesting = ref(false)
const vpnPreflightResult = ref<VpnPreflightResult | null>(null)

async function handleTestVpnPreflight() {
  vpnTesting.value = true
  try {
    vpnPreflightResult.value = await vpnStore.runPreflight()
  } finally {
    vpnTesting.value = false
  }
}
</script>
