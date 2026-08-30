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

    <!-- Barra de Ações em Lote -->
    <v-slide-y-transition>
      <div
        v-if="selectedDeviceIds.length > 0"
        class="d-flex align-center justify-space-between flex-wrap ga-2 pa-3 bg-surface-variant-subtle rounded-lg mb-4 elevation-1 border"
      >
        <div class="d-flex align-center ga-2">
          <v-chip color="primary" variant="flat" size="small" class="font-weight-bold">
            {{ selectedDeviceIds.length }} selecionado(s)
          </v-chip>
          <span class="text-body-2 font-weight-medium">Ações em lote para topologia:</span>
        </div>
        <div class="d-flex align-center ga-2">
          <v-btn
            color="primary"
            variant="elevated"
            size="small"
            prepend-icon="mdi-sitemap"
            @click="openBatchParentDialog"
          >
            Definir Dispositivo Pai
          </v-btn>
          <v-btn variant="text" size="small" @click="selectedDeviceIds = []">
            Limpar Seleção
          </v-btn>
        </div>
      </div>
    </v-slide-y-transition>

    <!-- Tabela de Dispositivos -->
    <v-card elevation="2" rounded="lg">
      <v-card-title class="pa-2.5 pa-sm-4 d-flex align-center">
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
        v-model="selectedDeviceIds"
        :headers="headers"
        :items="devicesStore.devices"
        :search="search"
        :loading="devicesStore.loading"
        :items-per-page="-1"
        hide-default-footer
        show-select
        item-value="id"
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

            <!--
              O dispositivo que representa esta instalação não é editável nem
              removível, e escanear as próprias portas não é uma ação válida.
              Quem responde isso é o backend, por `isSystem` — a tela nunca
              deduz pelo nome nem pela posição na lista. Um botão que só pode
              devolver erro é pior que botão nenhum.
            -->
            <template v-if="!item.isSystem">
              <v-btn
                icon
                size="small"
                variant="text"
                color="primary"
                @click.stop="openDialog(item)"
              >
                <v-icon>mdi-pencil</v-icon>
                <v-tooltip activator="parent" location="top">Editar</v-tooltip>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="purple"
                @click.stop="openPortScan(item)"
              >
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
            </template>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-2">
            <div class="d-flex align-start justify-space-between ga-2">
              <div class="flex-grow-1 min-w-0">
                <router-link
                  :to="'/devices/' + item.id"
                  class="text-subtitle-1 font-weight-bold text-decoration-none text-primary d-block text-truncate"
                  @click.stop
                >
                  {{ item.name }}
                </router-link>
                <div class="d-flex flex-wrap align-center ga-1 mt-1">
                  <span v-if="item.ipAddress" class="text-caption font-mono text-medium-emphasis">
                    {{ item.ipAddress }}
                  </span>
                  <v-chip
                    size="x-small"
                    color="info"
                    variant="tonal"
                    class="text-uppercase font-weight-medium"
                  >
                    {{ item.type }}
                  </v-chip>
                  <v-chip :color="getStatusColor(item.status)" size="x-small" variant="tonal">
                    <v-icon start size="10">mdi-circle</v-icon>
                    {{ (item.status || 'UNKNOWN').toUpperCase() }}
                  </v-chip>
                </div>
                <div
                  v-if="item.site || item.parent"
                  class="d-flex flex-wrap align-center ga-2 text-caption text-medium-emphasis mt-1"
                >
                  <span v-if="item.site" class="d-inline-flex align-center ga-1">
                    <v-icon size="13">mdi-map-marker-outline</v-icon>
                    {{ item.site.name }}
                  </span>
                  <span v-if="item.parent" class="d-inline-flex align-center ga-1">
                    <v-icon size="13">mdi-sitemap</v-icon>
                    {{ item.parent.name }}
                  </span>
                </div>
              </div>
            </div>

            <div class="d-flex align-center justify-end ga-1 pt-1 border-t mt-1">
              <v-btn
                icon
                size="small"
                variant="text"
                color="info"
                :to="'/devices/' + item.id"
                title="Ver detalhes"
                @click.stop
              >
                <v-icon size="18">mdi-eye</v-icon>
              </v-btn>
              <template v-if="!item.isSystem">
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="primary"
                  title="Editar"
                  @click.stop="openDialog(item)"
                >
                  <v-icon size="18">mdi-pencil</v-icon>
                </v-btn>
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="purple"
                  title="Escanear Portas"
                  @click.stop="openPortScan(item)"
                >
                  <v-icon size="18">mdi-lan-connect</v-icon>
                </v-btn>
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="error"
                  title="Excluir"
                  @click.stop="confirmDelete(item.id)"
                >
                  <v-icon size="18">mdi-delete</v-icon>
                </v-btn>
              </template>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <!-- Diálogo de Associação em Lote do Dispositivo Pai -->
    <v-dialog v-model="batchParentDialog" max-width="500">
      <v-card class="rounded-xl pa-4 elevation-12">
        <v-card-title class="font-weight-bold d-flex align-center ga-2 pa-0 mb-3">
          <v-icon color="primary">mdi-sitemap</v-icon>
          <span>Definir Dispositivo Pai em Lote</span>
        </v-card-title>
        <v-card-text class="pa-0 mb-4">
          <p class="text-body-2 text-medium-emphasis mb-3">
            Associar <strong>{{ selectedDeviceIds.length }}</strong> equipamento(s) selecionado(s) a
            um Switch ou Roteador pai para construção da topologia e inibição de alertas em cascata.
          </p>

          <v-select
            v-model="batchSelectedParentId"
            :items="parentCandidates"
            item-title="name"
            item-value="id"
            label="Dispositivo Pai (Uplink / Switch / Roteador)"
            placeholder="Selecione o equipamento ou deixe vazio para remover"
            variant="outlined"
            density="comfortable"
            clearable
            prepend-inner-icon="mdi-router-network"
          >
            <template #item="{ props: itemProps, item }">
              <v-list-item v-bind="itemProps" :title="item.name">
                <template #prepend>
                  <v-icon :color="item.isInfra ? 'primary' : 'grey'" size="20">
                    {{ item.icon }}
                  </v-icon>
                </template>
                <template #subtitle>
                  <div class="d-flex align-center ga-1 text-caption">
                    <span v-if="item.ipAddress" class="font-mono text-grey-darken-1">
                      {{ item.ipAddress }}
                    </span>
                    <span v-if="item.siteName" class="text-grey">• {{ item.siteName }}</span>
                  </div>
                </template>
                <template #append>
                  <v-chip
                    size="x-small"
                    :color="item.isInfra ? 'primary' : 'default'"
                    variant="tonal"
                    class="text-uppercase"
                  >
                    {{ item.type }}
                  </v-chip>
                </template>
              </v-list-item>
            </template>
          </v-select>
        </v-card-text>
        <v-card-actions class="pa-0 justify-end ga-2">
          <v-btn variant="text" :disabled="savingBatchParent" @click="batchParentDialog = false">
            Cancelar
          </v-btn>
          <v-btn
            color="primary"
            variant="elevated"
            :loading="savingBatchParent"
            @click="executeBatchSetParent"
          >
            Aplicar aos {{ selectedDeviceIds.length }} Dispositivo(s)
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

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
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useDevicesStore, type Device } from '@/stores/devices'
import DeviceDialog from '@/components/DeviceDialog.vue'
import PortScanDialog from '@/components/PortScanDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { getStatusColor } from '@/utils/monitorPresentation'
import { confirm } from '@/composables/useConfirm'

const router = useRouter()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const deviceToEdit = ref<Device | null>(null)

const selectedDeviceIds = ref<number[]>([])
const batchParentDialog = ref(false)
const batchSelectedParentId = ref<number | null>(null)
const savingBatchParent = ref(false)

const portScanDialog = ref(false)
const portScanHost = ref('')
const portScanDeviceName = ref('')

const INFRA_TYPES = new Set([
  'router',
  'switch',
  'firewall',
  'gateway',
  'unmanaged_switch',
  'ap',
  'access_point',
])

function getIcon(type?: string): string {
  switch (type?.toLowerCase()) {
    case 'router':
    case 'gateway':
      return 'mdi-router-network'
    case 'switch':
    case 'unmanaged_switch':
      return 'mdi-hub'
    case 'firewall':
      return 'mdi-shield-network'
    case 'ap':
    case 'access_point':
      return 'mdi-access-point'
    case 'server':
      return 'mdi-server'
    default:
      return 'mdi-lan'
  }
}

const parentCandidates = computed(() => {
  const selectedSet = new Set(selectedDeviceIds.value)
  return devicesStore.devices
    .filter((d) => !selectedSet.has(d.id))
    .map((d) => ({
      id: d.id,
      name: d.name,
      ipAddress: d.ipAddress || '',
      type: d.type,
      siteName: d.site?.name || '',
      isInfra: INFRA_TYPES.has(d.type?.toLowerCase() || ''),
      icon: getIcon(d.type),
    }))
    .sort((a, b) => {
      if (a.isInfra !== b.isInfra) return a.isInfra ? -1 : 1
      return a.name.localeCompare(b.name)
    })
})

function openBatchParentDialog() {
  batchSelectedParentId.value = null
  batchParentDialog.value = true
}

async function executeBatchSetParent() {
  if (selectedDeviceIds.value.length === 0) return
  savingBatchParent.value = true
  try {
    const ok = await devicesStore.batchSetParent(
      selectedDeviceIds.value,
      batchSelectedParentId.value
    )
    if (ok) {
      selectedDeviceIds.value = []
      batchParentDialog.value = false
    }
  } finally {
    savingBatchParent.value = false
  }
}

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
  const ok = await confirm({
    title: 'Excluir dispositivo',
    message:
      'Tem certeza de que deseja excluir este dispositivo? Todo o histórico de monitoramento associado será removido.',
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (ok) {
    await devicesStore.deleteDevice(id)
  }
}
</script>

<style scoped>
.row-pointer :deep(tbody tr) {
  cursor: pointer;
}
</style>
