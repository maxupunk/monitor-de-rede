<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-3">
      <div>
        <div class="text-h6 font-weight-bold">Primeira Sub-rede (Network CIDR)</div>
        <div class="text-caption text-high-emphasis">
          Cadastre a faixa de IP para que o NetMonitor descubra seus dispositivos.
        </div>
      </div>
      <v-switch
        v-model="model.enabled"
        color="primary"
        label="Cadastrar Sub-rede"
        density="compact"
        hide-details
      ></v-switch>
    </div>

    <v-expand-transition>
      <v-sheet v-if="model.enabled" border rounded="lg" class="pa-4 mb-4 bg-surface">
        <v-row dense>
          <v-col cols="12" sm="6">
            <v-text-field
              v-model="model.name"
              label="Nome da Rede *"
              placeholder="Ex: Rede Local Principal (LAN)"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-lan-connect"
              hint="Identificador da sub-rede"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12" sm="6">
            <v-text-field
              v-model="model.cidr"
              label="Faixa CIDR *"
              placeholder="Ex: 192.168.1.0/24"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-network"
              hint="Notação CIDR da rede (ex: 192.168.1.0/24)"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12" sm="6">
            <v-text-field
              v-model="model.gateway"
              label="Gateway Padrão (Roteador)"
              placeholder="Ex: 192.168.1.1"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-router-wireless"
              hint="IP do roteador ou switch gateway"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12" sm="6">
            <v-text-field
              v-model.number="model.vlanId"
              label="ID da VLAN (opcional)"
              type="number"
              placeholder="Ex: 10"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-tag-outline"
              hint="Deixe em branco se não usar VLAN tagged"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>

          <v-col cols="12">
            <v-divider class="my-2"></v-divider>
            <div class="d-flex flex-column ga-2 mt-2">
              <v-switch
                v-model="model.scanEnabled"
                color="primary"
                label="Ativar varredura periódica de novos dispositivos nesta rede"
                density="compact"
                hide-details
              ></v-switch>
              <v-switch
                v-model="model.scanNow"
                color="secondary"
                label="Disparar varredura imediata ao concluir o assistente (Recomendado)"
                density="compact"
                hide-details
              ></v-switch>
            </div>
          </v-col>
        </v-row>
      </v-sheet>
    </v-expand-transition>

    <v-alert type="info" variant="tonal" density="comfortable" class="rounded-lg">
      <div class="text-caption">
        A varredura testa cada IP da faixa enviando Ping e consultas SNMP para identificar
        fabricantes, nomes de host e portas ativas. Os equipamentos descobertos aparecerão em
        <strong>Descoberta</strong>.
      </div>
    </v-alert>
  </div>
</template>

<script setup lang="ts">
import type { NetworkForm } from './setupForm'

const model = defineModel<NetworkForm>({ required: true })
</script>
