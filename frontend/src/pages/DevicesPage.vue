<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Dispositivos</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Gerenciamento de equipamentos e servidores monitorados</p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
        Cadastrar Dispositivo
      </v-btn>
    </div>

    <!-- Tabela de Dispositivos -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 d-flex align-center gap-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, IP ou fornecedor"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="max-w-300"
        ></v-text-field>
      </v-card-title>

      <v-data-table
        :headers="headers"
        :items="devicesStore.devices"
        :search="search"
        :loading="devicesStore.loading"
        no-data-text="Nenhum dispositivo cadastrado"
      >
        <template #item.status="{ item }">
          <v-chip :color="getStatusColor(item.status)" size="small" variant="tonal">
            <v-icon start size="12">mdi-circle</v-icon>
            {{ (item.status || 'UNKNOWN').toUpperCase() }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <v-btn
            size="small"
            color="info"
            variant="tonal"
            prepend-icon="mdi-eye"
            class="mr-2"
            :to="`/devices/${item.id}`"
          >
            Detalhes
          </v-btn>

          <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>
      </v-data-table>
    </v-card>

    <!-- Modal Form de Dispositivo -->
    <v-dialog v-model="dialog" max-width="600">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editedId ? 'Editar Dispositivo' : 'Cadastrar Novo Dispositivo' }}
        </v-card-title>
        <v-card-text>
          <v-form @submit.prevent="save">
            <v-row>
              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.name"
                  label="Nome do Equipamento"
                  variant="outlined"
                  required
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.ipAddress"
                  label="Endereço IP"
                  variant="outlined"
                  required
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="6">
                <v-select
                  v-model="formModel.type"
                  :items="['router', 'switch', 'server', 'firewall', 'printer', 'ap', 'other']"
                  label="Tipo de Dispositivo"
                  variant="outlined"
                  required
                ></v-select>
              </v-col>
              <v-col cols="12" sm="6">
                <v-select
                  v-model="formModel.siteId"
                  :items="sitesStore.sites"
                  item-title="name"
                  item-value="id"
                  label="Site"
                  variant="outlined"
                  required
                ></v-select>
              </v-col>
              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.vendor"
                  label="Fabricante / Vendor"
                  placeholder="Cisco, MikroTik, Ubiquiti"
                  variant="outlined"
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.model"
                  label="Modelo"
                  variant="outlined"
                ></v-text-field>
              </v-col>
              <v-col cols="12">
                <v-checkbox
                  v-model="formModel.snmpEnabled"
                  label="Habilitar Coleta SNMP"
                  color="primary"
                  hide-details
                ></v-checkbox>
              </v-col>
              <v-col v-if="formModel.snmpEnabled" cols="12" sm="6">
                <v-text-field
                  v-model="formModel.snmpCommunity"
                  label="Comunidade SNMP"
                  variant="outlined"
                ></v-text-field>
              </v-col>
              <v-col v-if="formModel.snmpEnabled" cols="12" sm="6">
                <v-select
                  v-model="formModel.snmpVersion"
                  :items="['v1', 'v2c', 'v3']"
                  label="Versão SNMP"
                  variant="outlined"
                ></v-select>
              </v-col>
            </v-row>
          </v-form>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="dialog = false">Cancelar</v-btn>
          <v-btn color="primary" :loading="saving" @click="save">Salvar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useDevicesStore, type Device } from '@/stores/devices'
import { useSitesStore } from '@/stores/sites'

const devicesStore = useDevicesStore()
const sitesStore = useSitesStore()
const search = ref('')
const dialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{
  name: string
  ipAddress: string
  type: string
  siteId: number
  vendor: string
  model: string
  snmpEnabled: boolean
  snmpCommunity: string
  snmpVersion: 'v1' | 'v2c' | 'v3'
}>({
  name: '',
  ipAddress: '',
  type: 'router',
  siteId: 1,
  vendor: '',
  model: '',
  snmpEnabled: false,
  snmpCommunity: 'public',
  snmpVersion: 'v2c',
})

const headers = [
  { title: 'ID', key: 'id', width: '60px' },
  { title: 'Nome', key: 'name' },
  { title: 'IP', key: 'ipAddress' },
  { title: 'Tipo', key: 'type' },
  { title: 'Fabricante', key: 'vendor' },
  { title: 'Status', key: 'status', width: '120px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '200px' },
]

onMounted(async () => {
  await Promise.all([devicesStore.fetchDevices(), sitesStore.fetchSites()])
})

function getStatusColor(status: string) {
  switch (status) {
    case 'online': return 'success'
    case 'offline': return 'error'
    case 'warning': return 'warning'
    default: return 'grey'
  }
}

function openDialog(device?: Device) {
  if (device) {
    editedId.value = device.id
    formModel.name = device.name
    formModel.ipAddress = device.ipAddress || ''
    formModel.type = device.type || 'router'
    formModel.siteId = device.siteId
    formModel.vendor = device.vendor || ''
    formModel.model = device.model || ''
    formModel.snmpEnabled = Boolean(device.snmpEnabled)
    formModel.snmpCommunity = device.snmpCommunity || 'public'
    formModel.snmpVersion = device.snmpVersion || 'v2c'
  } else {
    editedId.value = null
    formModel.name = ''
    formModel.ipAddress = ''
    formModel.type = 'router'
    formModel.siteId = sitesStore.sites[0]?.id || 1
    formModel.vendor = ''
    formModel.model = ''
    formModel.snmpEnabled = false
    formModel.snmpCommunity = 'public'
    formModel.snmpVersion = 'v2c'
  }
  dialog.value = true
}

async function save() {
  if (!formModel.name || !formModel.ipAddress) return
  saving.value = true
  if (editedId.value) {
    await devicesStore.updateDevice(editedId.value, formModel)
  } else {
    await devicesStore.createDevice(payloadForCreate())
  }
  saving.value = false
  dialog.value = false
}

function payloadForCreate() {
  return {
    ...formModel,
    status: 'unknown' as const,
  }
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir este dispositivo?')) {
    await devicesStore.deleteDevice(id)
  }
}
</script>
