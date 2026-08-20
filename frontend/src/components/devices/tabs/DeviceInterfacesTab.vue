<template>
  <div>
    <div class="text-caption text-grey mb-3">
      Clique em uma interface para ver o histórico de tráfego e incluí-la ou removê-la do
      monitoramento.
    </div>
    <div class="table-responsive">
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
