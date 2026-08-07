<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Monitores de Rede</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Verificações de Ping, HTTP, TCP, DNS e SNMP com histórico em linha do tempo
        </p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()"> Novo Monitor </v-btn>
    </div>

    <!-- Tabela de Monitores -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 d-flex align-center ga-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, tipo ou alvo"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="max-w-300"
        ></v-text-field>
      </v-card-title>

      <MonitorsTable
        :monitors="monitorsStore.monitors"
        :loading="monitorsStore.loading"
        :search="search"
        variant="full"
        @edit="openDialog"
        @changed="refresh"
      ></MonitorsTable>
    </v-card>

    <!-- Modal Form de Monitor -->
    <MonitorFormDialog
      v-model="dialog"
      :monitor="editingMonitor"
      @saved="refresh"
    ></MonitorFormDialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'
import MonitorsTable from '@/components/MonitorsTable.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const editingMonitor = ref<Monitor | null>(null)

onMounted(async () => {
  await Promise.all([monitorsStore.fetchMonitors(), devicesStore.fetchDevices()])
})

function openDialog(monitor?: Monitor) {
  editingMonitor.value = monitor ?? null
  dialog.value = true
}

async function refresh() {
  await monitorsStore.fetchMonitors()
}
</script>
