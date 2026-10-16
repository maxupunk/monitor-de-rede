<template>
  <div class="topology-state">
    <div v-if="loading" class="text-center">
      <v-progress-circular indeterminate color="primary" size="48"></v-progress-circular>
      <div class="mt-3 font-weight-bold">Carregando o mapa…</div>
    </div>
    <v-card v-else class="state-card pa-6 text-center" elevation="8" border max-width="520">
      <template v-if="error">
        <v-avatar color="error" size="72" class="mb-4">
          <v-icon size="40" color="white">mdi-lan-disconnect</v-icon>
        </v-avatar>
        <div class="text-h6 font-weight-bold mb-1">Não foi possível carregar o mapa</div>
        <p class="text-body-2 mb-5">{{ error }}</p>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-refresh" @click="$emit('retry')">
          Tentar de novo
        </v-btn>
      </template>

      <template v-else-if="filteredBySite">
        <v-avatar color="info" size="72" class="mb-4">
          <v-icon size="40" color="white">mdi-office-building-marker</v-icon>
        </v-avatar>
        <div class="text-h6 font-weight-bold mb-1">Nenhum dispositivo neste site</div>
        <p class="text-body-2 mb-5">
          Escolha outro site ou veja a rede inteira. Dispositivos ganham site no cadastro.
        </p>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-earth" @click="$emit('all-sites')">
          Ver todos os sites
        </v-btn>
      </template>

      <template v-else>
        <div class="illustration mb-4" aria-hidden="true">
          <v-icon size="44" color="primary" class="hub">mdi-router-network</v-icon>
          <v-icon size="30" color="secondary" class="leaf leaf-a">mdi-switch</v-icon>
          <v-icon size="30" color="success" class="leaf leaf-b">mdi-server</v-icon>
          <v-icon size="30" color="info" class="leaf leaf-c">mdi-access-point</v-icon>
        </div>
        <div class="text-h6 font-weight-bold mb-1">Seu mapa de rede começa aqui</div>
        <p class="text-body-2 mb-5">
          Cadastre ou descubra dispositivos e o NetMonitor desenha as ligações entre eles — por
          LLDP/CDP, pela sub-rede ou pelos cabos que você registrar.
        </p>
        <div class="d-flex flex-column ga-2 align-stretch">
          <v-tooltip :text="READ_ONLY_REASON" :disabled="canWrite">
            <template #activator="{ props: tip }">
              <span v-bind="tip">
                <v-btn
                  color="primary"
                  variant="flat"
                  block
                  prepend-icon="mdi-radar"
                  :loading="discovering"
                  :disabled="!canWrite"
                  @click="$emit('discover')"
                >
                  Descobrir enlaces (LLDP / sub-rede)
                </v-btn>
              </span>
            </template>
          </v-tooltip>
          <v-btn color="info" variant="flat" block prepend-icon="mdi-plus-network" to="/discovery">
            Encontrar dispositivos na rede
          </v-btn>
          <div class="d-flex ga-2 flex-wrap">
            <v-btn
              v-if="canWrite"
              color="primary"
              variant="outlined"
              class="flex-grow-1"
              prepend-icon="mdi-hub"
              @click="$emit('add-switch')"
            >
              Switch não gerenciável
            </v-btn>
            <v-btn
              color="primary"
              variant="outlined"
              class="flex-grow-1"
              prepend-icon="mdi-devices"
              to="/devices"
            >
              Cadastrar dispositivo
            </v-btn>
          </div>
        </div>
      </template>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'

defineProps<{
  loading?: boolean
  error?: string | null
  filteredBySite?: boolean
  canWrite: boolean
  discovering?: boolean
}>()

defineEmits<{ retry: []; discover: []; 'add-switch': []; 'all-sites': [] }>()
</script>

<style scoped>
.topology-state {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  z-index: 3;
}
.topology-state > .text-center {
  color: rgb(var(--v-theme-on-surface));
}
.state-card {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
}
.illustration {
  position: relative;
  width: 160px;
  height: 110px;
  margin: 0 auto;
}
.illustration::before {
  content: '';
  position: absolute;
  left: 30px;
  right: 30px;
  top: 34px;
  height: 46px;
  border: 2px dashed rgba(var(--v-theme-primary), 0.6);
  border-bottom: none;
  border-radius: 12px 12px 0 0;
}
.hub {
  position: absolute;
  left: 58px;
  top: 0;
}
.leaf {
  position: absolute;
  bottom: 0;
}
.leaf-a {
  left: 14px;
}
.leaf-b {
  left: 65px;
}
.leaf-c {
  right: 14px;
}
</style>
