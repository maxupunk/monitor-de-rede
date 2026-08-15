<template>
  <div>
    <PageHeader
      title="Dispositivos"
      subtitle="Gerenciamento de equipamentos e servidores monitorados"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
          <span class="hidden-sm-and-down">Cadastrar Dispositivo</span>
          <span class="hidden-md-and-up">Novo</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Tabela de Dispositivos -->
    <v-card elevation="2" rounded="lg">
      <v-card-title class="pa-4 d-flex align-center">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, IP ou fornecedor"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="w-100"
          style="max-width: 420px"
        ></v-text-field>
      </v-card-title>

      <ResponsiveDataTable
        :headers="headers"
        :items="devicesStore.devices"
        :search="search"
        :loading="devicesStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum dispositivo cadastrado"
        clickable
        @click:row="onRowClick"
      >
        <template #item.name="{ item }">
          <router-link
            :to="'/devices/' + item.id"
            class="text-decoration-none text-primary font-weight-medium"
            @click.stop
          >
            {{ item.name }}
          </router-link>
        </template>

        <template #item.site="{ item }">
          <span>{{ item.site ? item.site.name : '-' }}</span>
        </template>

        <template #item.parent="{ item }">
          <span>{{ item.parent ? item.parent.name : '-' }}</span>
        </template>

        <template #item.isMonitored="{ item }">
          <v-chip :color="item.isMonitored ? 'success' : 'grey'" size="small" variant="tonal">
            {{ item.isMonitored ? 'SIM' : 'NÃO' }}
          </v-chip>
        </template>

        <template #item.status="{ item }">
          <v-chip :color="getStatusColor(item.status)" size="small" variant="tonal">
            <v-icon start size="12">mdi-circle</v-icon>
            {{ (item.status || 'UNKNOWN').toUpperCase() }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex ga-1">
            <v-btn
              icon
              size="small"
              variant="text"
              color="info"
              :to="'/devices/' + item.id"
              @click.stop
            >
              <v-icon>mdi-eye</v-icon>
              <v-tooltip activator="parent" location="top">Detalhes</v-tooltip>
            </v-btn>

            <v-btn icon size="small" variant="text" color="primary" @click.stop="openDialog(item)">
              <v-icon>mdi-pencil</v-icon>
              <v-tooltip activator="parent" location="top">Editar</v-tooltip>
            </v-btn>
            <v-btn icon size="small" variant="text" color="purple" @click.stop="openPortScan(item)">
              <v-icon>mdi-lan-connect</v-icon>
              <v-tooltip activator="parent" location="top">Escanear Portas</v-tooltip>
            </v-btn>
            <v-btn
              icon
              size="small"
              variant="text"
              color="error"
              @click.stop="confirmDelete(item.id)"
            >
              <v-icon>mdi-delete</v-icon>
              <v-tooltip activator="parent" location="top">Excluir</v-tooltip>
            </v-btn>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div class="flex-grow-1 text-break">
              <router-link
                :to="'/devices/' + item.id"
                class="text-subtitle-1 font-weight-bold text-decoration-none text-primary d-block"
                @click.stop
              >
                {{ item.name }}
              </router-link>
              <div class="d-flex flex-wrap align-center ga-2 mt-1">
                <span class="text-caption text-grey-darken-1">{{ item.ipAddress }}</span>
                <v-chip size="x-small" color="info" variant="tonal">{{ item.type }}</v-chip>
                <v-chip :color="getStatusColor(item.status)" size="x-small" variant="tonal">
                  {{ (item.status || 'UNKNOWN').toUpperCase() }}
                </v-chip>
              </div>
              <div v-if="item.site" class="text-caption text-grey mt-1">
                {{ item.site.name }}
              </div>
            </div>
            <div class="d-flex flex-column ga-1">
              <v-btn icon size="small" variant="text" color="info" :to="'/devices/' + item.id">
                <v-icon>mdi-eye</v-icon>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="primary"
                @click.stop="openDialog(item)"
              >
                <v-icon>mdi-pencil</v-icon>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="purple"
                @click.stop="openPortScan(item)"
              >
                <v-icon>mdi-lan-connect</v-icon>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="error"
                @click.stop="confirmDelete(item.id)"
              >
                <v-icon>mdi-delete</v-icon>
              </v-btn>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <!-- Componente Reusável Modal de Dispositivo -->
    <DeviceDialog v-model="dialog" :device-to-edit="deviceToEdit" @saved="onDeviceSaved" />

    <!-- Componente Reusável Modal de Scanner de Portas -->
    <PortScanDialog
      v-model="portScanDialog"
      :host="portScanHost"
      :device-name="portScanDeviceName"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useDevicesStore, type Device } from '@/stores/devices'
import DeviceDialog from '@/components/DeviceDialog.vue'
import PortScanDialog from '@/components/PortScanDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { getStatusColor } from '@/utils/monitorPresentation'

const router = useRouter()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const deviceToEdit = ref<Device | null>(null)

const portScanDialog = ref(false)
const portScanHost = ref('')
const portScanDeviceName = ref('')

const headers = [
  { title: 'ID', key: 'id', width: '60px' },
  { title: 'Nome', key: 'name' },
  { title: 'IP', key: 'ipAddress' },
  { title: 'Tipo', key: 'type' },
  { title: 'Site', key: 'site' },
  { title: 'Está atrás de', key: 'parent' },
  { title: 'Monitorado', key: 'isMonitored', width: '100px' },
  { title: 'Status', key: 'status', width: '120px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '140px' },
]

onMounted(async () => {
  await devicesStore.fetchDevices()
})

function onRowClick(_event: MouseEvent, row: { item: Device }) {
  if (row?.item?.id) {
    router.push('/devices/' + row.item.id)
  }
}

function openDialog(device?: Device) {
  deviceToEdit.value = device || null
  dialog.value = true
}

function onDeviceSaved() {
  devicesStore.fetchDevices()
}

function openPortScan(device: Device) {
  portScanHost.value = device.ipAddress || ''
  portScanDeviceName.value = device.name
  portScanDialog.value = true
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir este dispositivo?')) {
    await devicesStore.deleteDevice(id)
  }
}
</script>

<style scoped>
.row-pointer :deep(tbody tr) {
  cursor: pointer;
}
</style>
