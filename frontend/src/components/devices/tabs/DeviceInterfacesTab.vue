<template>
  <div>
    <div class="text-caption text-grey mb-3">
      Clique em uma interface para ver o histórico de tráfego e incluí-la ou removê-la do
      monitoramento.
    </div>
    <!-- Desktop: Tabela -->
    <div v-if="$vuetify.display.mdAndUp" class="table-responsive">
      <v-table hover>
        <thead>
          <tr>
            <th>Index</th>
            <th>Nome Interface</th>
            <th>Monitoramento</th>
            <th>Status Operacional</th>
            <th>MAC Address</th>
            <th>Velocidade de Negociação</th>
            <th style="width: 56px"></th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="intf in detailStore.interfaces"
            :key="intf.id"
            class="cursor-pointer"
            @click="emit('openInterfaceChart', intf)"
          >
            <td>{{ intf.ifIndex ?? intf.snmpIndex ?? '-' }}</td>
            <td class="font-weight-bold">{{ interfaceLabel(intf) }}</td>
            <td>
              <v-chip :color="intf.isMonitored ? 'primary' : 'grey'" size="x-small" variant="tonal">
                {{ intf.isMonitored ? 'MONITORADA' : 'NÃO MONITORADA' }}
              </v-chip>
            </td>
            <td>
              <v-chip
                :color="(intf.ifOperStatus || intf.operStatus) === 'up' ? 'success' : 'error'"
                size="x-small"
              >
                Oper: {{ intf.ifOperStatus || intf.operStatus || 'unknown' }}
              </v-chip>
            </td>
            <td>{{ intf.macAddress || 'N/A' }}</td>
            <td>
              <v-chip size="x-small" variant="tonal" color="info">
                {{ formatLinkSpeed(intf.ifSpeed || intf.speed) }}
              </v-chip>
            </td>
            <td>
              <v-btn icon size="x-small" variant="text" color="primary">
                <v-icon size="18">mdi-chart-line</v-icon>
                <v-tooltip activator="parent" location="top">
                  Ver gráficos e gerenciar monitoramento
                </v-tooltip>
              </v-btn>
            </td>
          </tr>
          <tr v-if="detailStore.interfaces.length === 0">
            <td colspan="7" class="text-center text-grey py-4">
              Nenhuma interface SNMP registrada ainda. Use "Configurar Monitoramento" para
              descobri-las.
            </td>
          </tr>
        </tbody>
      </v-table>
    </div>

    <!-- Mobile: Cards Responsivos -->
    <div v-else class="d-flex flex-column ga-2">
      <template v-if="detailStore.interfaces.length > 0">
        <v-card
          v-for="intf in detailStore.interfaces"
          :key="intf.id"
          border
          rounded="lg"
          class="pa-3 cursor-pointer"
          @click="emit('openInterfaceChart', intf)"
        >
          <div class="d-flex align-center justify-space-between ga-2">
            <div class="d-flex align-center ga-2 min-w-0">
              <span class="text-caption font-mono text-medium-emphasis"
              >#{{ intf.ifIndex ?? intf.snmpIndex ?? '-' }}</span
              >
              <span class="font-weight-bold text-subtitle-1 text-truncate">{{
                interfaceLabel(intf)
              }}</span>
            </div>
            <div class="d-flex align-center ga-1.5 flex-shrink-0">
              <v-chip
                :color="(intf.ifOperStatus || intf.operStatus) === 'up' ? 'success' : 'error'"
                size="x-small"
                variant="flat"
                class="font-weight-bold text-uppercase px-2"
              >
                {{ (intf.ifOperStatus || intf.operStatus || 'UNKNOWN').toUpperCase() }}
              </v-chip>
              <v-icon size="18" color="grey">mdi-chevron-right</v-icon>
            </div>
          </div>

          <div class="d-flex flex-wrap align-center ga-2 mt-1">
            <v-chip :color="intf.isMonitored ? 'primary' : 'grey'" size="x-small" variant="tonal">
              {{ intf.isMonitored ? 'MONITORADA' : 'NÃO MONITORADA' }}
            </v-chip>
            <v-chip size="x-small" variant="tonal" color="info">
              {{ formatLinkSpeed(intf.ifSpeed || intf.speed) }}
            </v-chip>
            <span v-if="intf.macAddress" class="text-caption font-mono text-medium-emphasis">
              {{ intf.macAddress }}
            </span>
          </div>
        </v-card>
      </template>

      <v-card v-else variant="outlined" rounded="lg" class="pa-6 text-center text-grey">
        <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-expansion-card</v-icon>
        <div class="text-subtitle-2 font-weight-medium">
          Nenhuma interface SNMP registrada ainda. Use "Configurar Monitoramento" para descobri-las.
        </div>
      </v-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useDeviceDetailStore, type DeviceInterface } from '@/stores/deviceDetail'
import { formatLinkSpeed } from '@/utils/formatters'

const emit = defineEmits<{
  (e: 'openInterfaceChart', intf: DeviceInterface): void
}>()

const detailStore = useDeviceDetailStore()

function interfaceLabel(intf: DeviceInterface): string {
  return intf.ifName || intf.name || `if-${intf.id}`
}
</script>
