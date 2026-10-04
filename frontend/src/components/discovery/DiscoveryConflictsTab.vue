<template>
  <div>
    <v-alert
      :type="hostMode ? 'success' : 'warning'"
      variant="tonal"
      density="comfortable"
      rounded="lg"
      class="mb-4"
      :prepend-icon="hostMode ? 'mdi-check-decagram' : 'mdi-alert-circle-outline'"
    >
      <div class="text-subtitle-2 font-weight-bold">
        {{ hostMode ? 'Modo de rede host ativo' : 'Ambiente em rede bridge (isolada)' }}
      </div>
      <div v-if="hostMode" class="text-body-2">
        O container enxerga a camada 2 (ARP): clones, conflitos de IP e MACs duplicados são
        detectados direto na rede física.
      </div>
      <div v-else class="text-body-2">
        Em modo bridge o Docker esconde a camada 2 atrás do NAT. Para detectar conflitos ARP na LAN
        física, use <code>docker-compose.host.yml</code> com <code>network_mode: host</code>.
      </div>
    </v-alert>

    <div class="d-flex flex-wrap align-center justify-space-between ga-2 mb-4">
      <div>
        <div class="text-subtitle-1 font-weight-bold d-flex align-center ga-2">
          <v-icon color="primary">mdi-shield-alert-outline</v-icon>
          Conflitos e clones de rede
        </div>
        <div class="text-body-2">
          Vários MACs respondendo pelo mesmo IP, ou o mesmo MAC em vários IPs.
        </div>
      </div>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-shield-search"
        :loading="loading"
        @click="emit('check')"
      >
        Verificar agora
      </v-btn>
    </div>

    <div v-if="conflicts.length === 0" class="text-center pa-6">
      <v-icon size="44" color="success" class="mb-2">mdi-shield-check</v-icon>
      <div class="text-subtitle-1 font-weight-bold">Nenhum conflito ou clone detectado</div>
      <div class="text-body-2">Os IPs e MACs da tabela de vizinhos estão consistentes.</div>
    </div>

    <div v-else class="d-flex flex-column ga-3">
      <v-card
        v-for="conflict in conflicts"
        :key="conflict.id"
        variant="outlined"
        rounded="lg"
        class="pa-4"
      >
        <div class="d-flex flex-wrap align-center justify-space-between ga-2 mb-2">
          <div class="d-flex align-center ga-2">
            <v-chip
              :color="conflict.severity === 'critical' ? 'error' : 'warning'"
              size="small"
              variant="flat"
              class="font-weight-bold"
            >
              {{ conflictTypeLabel(conflict.conflictType) }}
            </v-chip>
            <span class="text-subtitle-2 font-mono font-weight-bold">
              {{ conflict.ipAddress }}
            </span>
          </div>
          <span class="text-caption">{{ formatDateTime(conflict.detectedAt) }}</span>
        </div>

        <div class="text-body-2 mb-2">{{ conflict.description }}</div>

        <div class="d-flex flex-wrap align-center ga-3 text-body-2">
          <div class="d-flex align-center ga-1">
            <v-icon size="16">mdi-ethernet</v-icon>
            <span>MAC(s):</span>
            <span class="font-mono font-weight-medium">{{ conflict.macAddresses.join(', ') }}</span>
          </div>
          <div v-if="conflict.vendors.length > 0" class="d-flex align-center ga-1">
            <v-icon size="16">mdi-domain</v-icon>
            <span>Fabricante(s):</span>
            <span class="font-weight-medium">{{ conflict.vendors.join(', ') }}</span>
          </div>
          <div v-if="conflict.affectedDeviceName" class="d-flex align-center ga-1">
            <v-icon size="16">mdi-server-network</v-icon>
            <span>Dispositivo:</span>
            <router-link
              :to="'/devices/' + conflict.affectedDeviceId"
              class="text-decoration-none font-weight-medium text-primary"
            >
              {{ conflict.affectedDeviceName }}
            </router-link>
          </div>
        </div>
      </v-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { NetworkConflict } from '@/stores/discovery'
import { formatDateTime } from '@/utils/formatters'

defineProps<{
  conflicts: NetworkConflict[]
  hostMode: boolean
  loading: boolean
}>()

const emit = defineEmits<{ check: [] }>()

const CONFLICT_LABELS: Record<NetworkConflict['conflictType'], string> = {
  ipCollision: 'IP clonado / colisão',
  macDuplicated: 'MAC duplicado / clonado',
  deviceMacMismatch: 'Divergência de cadastro',
}

function conflictTypeLabel(type: NetworkConflict['conflictType']): string {
  return CONFLICT_LABELS[type] ?? 'Conflito de rede'
}
</script>
