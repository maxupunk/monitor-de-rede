<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Dispositivos</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Gerenciamento de equipamentos e servidores monitorados
        </p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
        Cadastrar Dispositivo
      </v-btn>
    </div>

    <!-- Tabela de Dispositivos -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 d-flex align-center ga-4">
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
        hover
        class="row-pointer"
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
        </template>
      </v-data-table>
    </v-card>

    <!-- Modal Form de Dispositivo -->
    <v-dialog v-model="dialog" max-width="650">
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
                  label="Nome do Equipamento *"
                  variant="outlined"
                  density="comfortable"
                  required
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.ipAddress"
                  label="Endereço IP *"
                  placeholder="Ex: 192.168.1.1"
                  variant="outlined"
                  density="comfortable"
                  required
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="6">
                <v-select
                  v-model="formModel.type"
                  :items="['router', 'switch', 'server', 'firewall', 'printer', 'ap', 'other']"
                  label="Tipo de Dispositivo"
                  variant="outlined"
                  density="comfortable"
                  required
                ></v-select>
              </v-col>

              <!-- Seleção de Site Opcional com Botão para Novo Site -->
              <v-col cols="12" sm="6">
                <div class="d-flex align-center ga-2">
                  <v-select
                    v-model="formModel.siteId"
                    :items="sitesStore.sites"
                    item-title="name"
                    item-value="id"
                    label="Site (Opcional)"
                    variant="outlined"
                    density="comfortable"
                    clearable
                    hide-details
                    class="flex-grow-1"
                  ></v-select>
                  <v-btn
                    icon
                    color="primary"
                    variant="tonal"
                    density="comfortable"
                    @click="siteDialog = true"
                  >
                    <v-icon>mdi-plus</v-icon>
                    <v-tooltip activator="parent" location="top">Cadastrar Novo Site</v-tooltip>
                  </v-btn>
                </div>
              </v-col>

              <!-- Campo "Está atrás de" para Topologia -->
              <v-col cols="12" sm="6">
                <v-select
                  v-model="formModel.parentId"
                  :items="availableParentDevices"
                  item-title="name"
                  item-value="id"
                  label="Está atrás de (Dispositivo Pai)"
                  variant="outlined"
                  density="comfortable"
                  clearable
                  hint="Indica a qual equipamento (ex: Switch/Roteador) este dispositivo está conectado."
                  persistent-hint
                >
                  <template #append-inner>
                    <v-icon size="small" color="grey-darken-1">mdi-help-circle-outline</v-icon>
                    <v-tooltip activator="parent" location="top">
                      Esta associação mapeia o caminho físico da rede para montar a estrutura de
                      topologia.
                    </v-tooltip>
                  </template>
                </v-select>
              </v-col>

              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.vendor"
                  label="Fabricante / Vendor"
                  placeholder="Cisco, MikroTik, Ubiquiti"
                  variant="outlined"
                  density="comfortable"
                ></v-text-field>
              </v-col>

              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="formModel.model"
                  label="Modelo"
                  variant="outlined"
                  density="comfortable"
                ></v-text-field>
              </v-col>

              <!-- Opção de Monitorar -->
              <v-col cols="12">
                <v-checkbox
                  v-model="formModel.isMonitored"
                  label="Monitorar este dispositivo (Disponível em /monitors)"
                  color="primary"
                  hide-details
                ></v-checkbox>
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
                  density="comfortable"
                ></v-text-field>
              </v-col>
              <v-col v-if="formModel.snmpEnabled" cols="12" sm="6">
                <v-select
                  v-model="formModel.snmpVersion"
                  :items="['v1', 'v2c', 'v3']"
                  label="Versão SNMP"
                  variant="outlined"
                  density="comfortable"
                ></v-select>
              </v-col>
              <v-col v-if="formModel.snmpEnabled" cols="12">
                <div class="d-flex align-center ga-2">
                  <v-select
                    v-model="formModel.zabbixTemplateId"
                    :items="zabbixTemplatesStore.templates"
                    item-title="name"
                    item-value="id"
                    label="Template Zabbix (Opcional)"
                    hint="Define quais OIDs SNMP são coletados (tensão, corrente, etc.) além de CPU/Memória/Interfaces."
                    persistent-hint
                    variant="outlined"
                    density="comfortable"
                    clearable
                    class="flex-grow-1"
                  ></v-select>
                  <v-btn
                    icon
                    color="primary"
                    variant="tonal"
                    density="comfortable"
                    to="/zabbix-templates"
                  >
                    <v-icon>mdi-upload</v-icon>
                    <v-tooltip activator="parent" location="top">Importar Novo Template</v-tooltip>
                  </v-btn>
                </div>
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

    <!-- Componente Reusável Modal de Cadastro de Site -->
    <SiteDialog v-model="siteDialog" @saved="onSiteCreated" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useDevicesStore, type Device } from '@/stores/devices'
import { useSitesStore, type Site } from '@/stores/sites'
import { useZabbixTemplatesStore } from '@/stores/zabbixTemplates'
import SiteDialog from '@/components/SiteDialog.vue'

const router = useRouter()
const devicesStore = useDevicesStore()
const sitesStore = useSitesStore()
const zabbixTemplatesStore = useZabbixTemplatesStore()
const search = ref('')
const dialog = ref(false)
const siteDialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{
  name: string
  ipAddress: string
  type: string
  siteId: number | null
  parentId: number | null
  vendor: string
  model: string
  isMonitored: boolean
  snmpEnabled: boolean
  snmpCommunity: string
  snmpVersion: 'v1' | 'v2c' | 'v3'
  zabbixTemplateId: number | null
}>({
  name: '',
  ipAddress: '',
  type: 'router',
  siteId: null,
  parentId: null,
  vendor: '',
  model: '',
  isMonitored: true,
  snmpEnabled: false,
  snmpCommunity: 'public',
  snmpVersion: 'v2c',
  zabbixTemplateId: null,
})

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

const availableParentDevices = computed(() => {
  return devicesStore.devices.filter((d) => d.id !== editedId.value)
})

onMounted(async () => {
  await Promise.all([
    devicesStore.fetchDevices(),
    sitesStore.fetchSites(),
    zabbixTemplatesStore.fetchTemplates(),
  ])
})

function onRowClick(_event: MouseEvent, row: { item: Device }) {
  if (row?.item?.id) {
    router.push('/devices/' + row.item.id)
  }
}

function getStatusColor(status: string) {
  switch (status) {
    case 'online':
      return 'success'
    case 'offline':
      return 'error'
    case 'warning':
      return 'warning'
    default:
      return 'grey'
  }
}

function openDialog(device?: Device) {
  if (device) {
    editedId.value = device.id
    formModel.name = device.name
    formModel.ipAddress = device.ipAddress || ''
    formModel.type = device.type || 'router'
    formModel.siteId = device.siteId ?? null
    formModel.parentId = device.parentId ?? null
    formModel.vendor = device.vendor || ''
    formModel.model = device.model || ''
    formModel.isMonitored = Boolean(device.isMonitored)
    formModel.snmpEnabled = Boolean(device.snmpEnabled)
    formModel.snmpCommunity = device.snmpCommunity || 'public'
    formModel.snmpVersion = device.snmpVersion || 'v2c'
    formModel.zabbixTemplateId = device.zabbixTemplateId ?? null
  } else {
    editedId.value = null
    formModel.name = ''
    formModel.ipAddress = ''
    formModel.type = 'router'
    formModel.siteId = null
    formModel.parentId = null
    formModel.vendor = ''
    formModel.model = ''
    formModel.isMonitored = true
    formModel.snmpEnabled = false
    formModel.snmpCommunity = 'public'
    formModel.snmpVersion = 'v2c'
    formModel.zabbixTemplateId = null
  }
  dialog.value = true
}

function onSiteCreated(newSite: Site) {
  formModel.siteId = newSite.id
}

async function save() {
  if (!formModel.name || !formModel.ipAddress) return
  saving.value = true
  if (editedId.value) {
    await devicesStore.updateDevice(editedId.value, formModel)
  } else {
    await devicesStore.createDevice(payloadForCreate())
  }
  await devicesStore.fetchDevices()
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

<style scoped>
.row-pointer :deep(tbody tr) {
  cursor: pointer;
}
</style>
