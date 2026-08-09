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
      <v-card-title class="pa-4 d-flex align-center justify-space-between flex-wrap ga-3">
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

        <!-- Chips de Filtro por Status da Saúde Global -->
        <div class="d-flex align-center ga-1 flex-wrap">
          <v-chip
            :color="statusFilter === 'all' ? 'primary' : 'default'"
            :variant="statusFilter === 'all' ? 'flat' : 'outlined'"
            size="small"
            class="font-weight-medium cursor-pointer"
            @click="setStatusFilter('all')"
          >
            Todos ({{ counts.all }})
          </v-chip>
          <v-chip
            :color="statusFilter === 'up' ? 'success' : 'default'"
            :variant="statusFilter === 'up' ? 'flat' : 'outlined'"
            size="small"
            class="font-weight-medium cursor-pointer"
            @click="setStatusFilter('up')"
          >
            Up ({{ counts.up }})
          </v-chip>
          <v-chip
            :color="statusFilter === 'warning' ? 'warning' : 'default'"
            :variant="statusFilter === 'warning' ? 'flat' : 'outlined'"
            size="small"
            class="font-weight-medium cursor-pointer"
            @click="setStatusFilter('warning')"
          >
            Warning ({{ counts.warning }})
          </v-chip>
          <v-chip
            :color="statusFilter === 'down' ? 'error' : 'default'"
            :variant="statusFilter === 'down' ? 'flat' : 'outlined'"
            size="small"
            class="font-weight-medium cursor-pointer"
            @click="setStatusFilter('down')"
          >
            Down ({{ counts.down }})
          </v-chip>
          <v-chip
            :color="statusFilter === 'unknown' ? 'grey-darken-1' : 'default'"
            :variant="statusFilter === 'unknown' ? 'flat' : 'outlined'"
            size="small"
            class="font-weight-medium cursor-pointer"
            @click="setStatusFilter('unknown')"
          >
            Desconhecido ({{ counts.unknown }})
          </v-chip>
        </div>
      </v-card-title>

      <MonitorsTable
        :monitors="filteredMonitors"
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
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'
import MonitorsTable from '@/components/MonitorsTable.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import PageHeader from '@/components/PageHeader.vue'

const route = useRoute()
const router = useRouter()
const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()

const search = ref('')
const dialog = ref(false)
const editingMonitor = ref<Monitor | null>(null)

const statusFilter = computed(() => {
  const s = String(route.query.status || 'all').toLowerCase()
  if (['up', 'warning', 'down', 'unknown'].includes(s)) return s
  return 'all'
})

function setStatusFilter(val: string) {
  router.push({
    query: {
      ...route.query,
      status: val === 'all' ? undefined : val,
    },
  })
}

const counts = computed(() => {
  let up = 0
  let warning = 0
  let down = 0
  let unknown = 0

  for (const m of monitorsStore.monitors) {
    if (m.status === 'online' || m.status === 'up') up++
    else if (m.status === 'warning') warning++
    else if (m.status === 'offline' || m.status === 'down') down++
    else unknown++
  }

  return { all: monitorsStore.monitors.length, up, warning, down, unknown }
})

const filteredMonitors = computed(() => {
  const f = statusFilter.value
  if (f === 'all') return monitorsStore.monitors

  return monitorsStore.monitors.filter((m) => {
    if (f === 'up') return m.status === 'online' || m.status === 'up'
    if (f === 'warning') return m.status === 'warning'
    if (f === 'down') return m.status === 'offline' || m.status === 'down'
    if (f === 'unknown') return m.status === 'unknown' || !m.status
    return true
  })
})

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
