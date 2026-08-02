<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Monitores de Rede</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Gerenciamento de verificações ICMP (Ping), HTTP, TCP e DNS</p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
        Novo Monitor
      </v-btn>
    </div>

    <!-- Tabela de Monitores -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 d-flex align-center gap-4">
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

      <v-data-table
        :headers="headers"
        :items="monitorsStore.monitors"
        :search="search"
        :loading="monitorsStore.loading"
        no-data-text="Nenhum monitor cadastrado"
      >
        <template #item.type="{ item }">
          <v-chip size="small" color="info" variant="tonal">
            {{ (item.type || 'N/A').toUpperCase() }}
          </v-chip>
        </template>

        <template #item.status="{ item }">
          <v-chip :color="getStatusColor(item.status)" size="small">
            {{ (item.status || 'UNKNOWN').toUpperCase() }}
          </v-chip>
        </template>

        <template #item.isEnabled="{ item }">
          <v-switch
            :model-value="item.isEnabled"
            color="success"
            hide-details
            density="compact"
            @update:model-value="(val) => monitorsStore.toggleMonitorEnabled(item.id, Boolean(val))"
          ></v-switch>
        </template>

        <template #item.actions="{ item }">
          <v-btn
            size="small"
            color="secondary"
            variant="tonal"
            prepend-icon="mdi-play"
            class="mr-2"
            :loading="monitorsStore.runningId === item.id"
            @click="monitorsStore.runMonitor(item.id)"
          >
            Testar
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

    <!-- Modal Form de Monitor -->
    <v-dialog v-model="dialog" max-width="550">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editedId ? 'Editar Monitor' : 'Cadastrar Novo Monitor' }}
        </v-card-title>
        <v-card-text>
          <v-form @submit.prevent="save">
            <v-select
              v-model="formModel.deviceId"
              :items="devicesStore.devices"
              item-title="name"
              item-value="id"
              label="Dispositivo Associado"
              variant="outlined"
              required
            ></v-select>
            <v-text-field
              v-model="formModel.name"
              label="Nome do Monitor"
              placeholder="Ex: Ping Google DNS"
              variant="outlined"
              required
            ></v-text-field>
            <v-select
              v-model="formModel.type"
              :items="['ping', 'http', 'tcp', 'dns']"
              label="Tipo de Checagem"
              variant="outlined"
              required
            ></v-select>
            <v-text-field
              v-model="formModel.target"
              label="Alvo (IP / Hostname / URL)"
              placeholder="8.8.8.8 ou https://meusite.com"
              variant="outlined"
              required
            ></v-text-field>
            <v-text-field
              v-if="formModel.type === 'tcp'"
              v-model.number="formModel.port"
              label="Porta TCP"
              type="number"
              variant="outlined"
            ></v-text-field>
            <v-row>
              <v-col cols="6">
                <v-text-field
                  v-model.number="formModel.intervalSeconds"
                  label="Intervalo (s)"
                  type="number"
                  variant="outlined"
                ></v-text-field>
              </v-col>
              <v-col cols="6">
                <v-text-field
                  v-model.number="formModel.timeoutSeconds"
                  label="Timeout (s)"
                  type="number"
                  variant="outlined"
                ></v-text-field>
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
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{
  deviceId: number
  name: string
  type: 'ping' | 'http' | 'tcp' | 'dns'
  target: string
  port?: number
  intervalSeconds: number
  timeoutSeconds: number
}>({
  deviceId: 1,
  name: '',
  type: 'ping',
  target: '',
  port: 80,
  intervalSeconds: 60,
  timeoutSeconds: 5,
})

const headers = [
  { title: 'ID', key: 'id', width: '60px' },
  { title: 'Nome', key: 'name' },
  { title: 'Tipo', key: 'type', width: '100px' },
  { title: 'Alvo', key: 'target' },
  { title: 'Status', key: 'status', width: '110px' },
  { title: 'Ativo', key: 'isEnabled', width: '100px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

onMounted(async () => {
  await Promise.all([monitorsStore.fetchMonitors(), devicesStore.fetchDevices()])
})

function getStatusColor(status: string) {
  switch (status) {
    case 'online': return 'success'
    case 'offline': return 'error'
    case 'warning': return 'warning'
    default: return 'grey'
  }
}

function openDialog(monitor?: Monitor) {
  if (monitor) {
    editedId.value = monitor.id
    formModel.deviceId = monitor.deviceId
    formModel.name = monitor.name
    formModel.type = monitor.type
    formModel.target = monitor.target
    formModel.port = monitor.port
    formModel.intervalSeconds = monitor.intervalSeconds
    formModel.timeoutSeconds = monitor.timeoutSeconds
  } else {
    editedId.value = null
    formModel.deviceId = devicesStore.devices[0]?.id || 1
    formModel.name = ''
    formModel.type = 'ping'
    formModel.target = ''
    formModel.port = 80
    formModel.intervalSeconds = 60
    formModel.timeoutSeconds = 5
  }
  dialog.value = true
}

async function save() {
  if (!formModel.name || !formModel.target) return
  saving.value = true
  if (editedId.value) {
    await monitorsStore.updateMonitor(editedId.value, formModel)
  } else {
    await monitorsStore.createMonitor(formModel)
  }
  saving.value = false
  dialog.value = false
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir este monitor?')) {
    await monitorsStore.deleteMonitor(id)
  }
}
</script>
