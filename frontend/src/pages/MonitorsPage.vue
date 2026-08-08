<template>
  <div>
    <PageHeader
      title="Monitores de Rede"
      subtitle="Verificações de Ping, HTTP, TCP, DNS e SNMP com histórico em linha do tempo"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
          <span class="hidden-sm-and-down">Novo Monitor</span>
          <span class="hidden-md-and-up">Novo</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Tabela de Monitores -->
    <v-card elevation="2" class="mobile-full-bleed">
      <v-card-title class="pa-4 d-flex align-center">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, tipo ou alvo"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="w-100"
          style="max-width: 420px"
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
import PageHeader from '@/components/PageHeader.vue'

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
