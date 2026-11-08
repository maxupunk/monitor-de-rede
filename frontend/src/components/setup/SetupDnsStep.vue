<template>
  <div>
    <div class="mb-3">
      <div class="text-h6 font-weight-bold">Servidores DNS para Monitoramento & Benchmark</div>
      <div class="text-caption text-high-emphasis">
        Selecione quais resolvedores DNS você deseja monitorar continuamente para comparar latência
        e qualidade de navegação.
      </div>
    </div>

    <div class="text-overline text-high-emphasis mb-2">Provedores Públicos Populares</div>
    <v-row dense class="mb-4">
      <v-col v-for="dns in POPULAR_DNS_PRESETS" :key="dns.address" cols="12" sm="6" md="4">
        <ChoiceCard
          :selected="isDnsSelected(dns.address)"
          class="pa-3 h-100 d-flex flex-column justify-space-between"
          @click="toggleDnsPreset(dns)"
        >
          <div class="d-flex align-start ga-2">
            <v-checkbox-btn
              :model-value="isDnsSelected(dns.address)"
              color="primary"
              density="compact"
              class="mt-n1"
            ></v-checkbox-btn>
            <div>
              <div class="font-weight-bold text-subtitle-2">{{ dns.name }}</div>
              <div class="text-body-2 font-weight-medium">{{ dns.address }}</div>
              <div class="text-caption text-high-emphasis">{{ dns.description }}</div>
            </div>
          </div>
          <div class="d-flex justify-end mt-2">
            <v-chip size="x-small" variant="tonal" color="info">
              {{ dns.protocol.toUpperCase() }}
            </v-chip>
          </div>
        </ChoiceCard>
      </v-col>
    </v-row>

    <div class="text-overline text-high-emphasis mb-2">Adicionar DNS Personalizado (Opcional)</div>
    <v-sheet border rounded="lg" class="pa-4 bg-surface">
      <v-row dense>
        <v-col cols="12" sm="5">
          <v-text-field
            v-model="customDns.name"
            label="Nome do Servidor"
            placeholder="Ex: DNS Interno Matriz"
            variant="outlined"
            density="compact"
            hide-details="auto"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="5">
          <v-text-field
            v-model="customDns.address"
            label="Endereço IP ou URL DoH"
            placeholder="Ex: 192.168.1.1 ou 10.0.0.2"
            variant="outlined"
            density="compact"
            hide-details="auto"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="2" class="d-flex align-center">
          <v-btn
            color="primary"
            variant="tonal"
            block
            prepend-icon="mdi-plus"
            :disabled="!customDns.name || !customDns.address"
            @click="addCustomDns"
          >
            Adicionar
          </v-btn>
        </v-col>
      </v-row>
    </v-sheet>
  </div>
</template>

<script setup lang="ts">
import { reactive } from 'vue'
import ChoiceCard from '@/components/ChoiceCard.vue'
import type { DnsServerPayload } from '@/stores/dnsServers'
import { POPULAR_DNS_PRESETS } from './setupForm'

/** Os servidores DNS escolhidos (presets e personalizados). */
const model = defineModel<DnsServerPayload[]>({ required: true })

const customDns = reactive({
  name: '',
  address: '',
})

function isDnsSelected(address: string): boolean {
  return model.value.some((d) => d.address === address)
}

function toggleDnsPreset(preset: DnsServerPayload) {
  model.value = isDnsSelected(preset.address)
    ? model.value.filter((d) => d.address !== preset.address)
    : [...model.value, { ...preset }]
}

function addCustomDns() {
  if (!customDns.name.trim() || !customDns.address.trim()) return
  const isDoh = customDns.address.startsWith('https://')
  model.value = [
    ...model.value,
    {
      name: customDns.name.trim(),
      address: customDns.address.trim(),
      protocol: isDoh ? 'doh' : 'udp',
      isDefault: true,
      description: 'DNS personalizado adicionado no assistente',
    },
  ]
  customDns.name = ''
  customDns.address = ''
}
</script>
