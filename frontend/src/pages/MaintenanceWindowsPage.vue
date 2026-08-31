<template>
  <div>
    <PageHeader
      title="Janelas de Manutenção"
      subtitle="Agende intervalos em que alertas e notificações de um site ou dispositivo ficam silenciados"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
          <span class="hidden-sm-and-down">Nova Janela</span>
          <span class="hidden-md-and-up">Novo</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Tabela de Janelas de Manutenção -->
    <v-card elevation="2" rounded="lg">
      <div class="pa-3 pa-sm-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome ou descrição"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="w-100"
          style="max-width: 420px"
        />
      </div>

      <ResponsiveDataTable
        :headers="headers"
        :items="windowsStore.windows"
        :search="search"
        :loading="windowsStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhuma janela de manutenção cadastrada"
        :clickable="false"
      >
        <template #item.scope="{ item }">
          <span v-if="item.siteId">{{ siteName(item.siteId) }}</span>
          <span v-else-if="item.deviceId">{{ deviceName(item.deviceId) }}</span>
          <span v-else class="text-grey">—</span>
        </template>

        <template #item.period="{ item }">
          {{ formatDateTime(item.startsAt) }} — {{ formatDateTime(item.endsAt) }}
        </template>

        <template #item.status="{ item }">
          <v-chip :color="isActive(item) ? 'warning' : 'success'" size="small" variant="tonal">
            {{
              isActive(item)
                ? 'Em vigor'
                : item.endsAt && new Date(item.endsAt) < now
                  ? 'Encerrada'
                  : 'Agendada'
            }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex ga-1">
            <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
              <v-icon>mdi-pencil</v-icon>
            </v-btn>
            <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
              <v-icon>mdi-delete</v-icon>
            </v-btn>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-2">
            <!-- Top Row: Nome e Status Chip -->
            <div class="d-flex align-center justify-space-between ga-2">
              <span class="text-subtitle-1 font-weight-bold text-truncate">{{ item.name }}</span>
              <v-chip
                :color="isActive(item) ? 'warning' : 'success'"
                size="small"
                variant="tonal"
                class="font-weight-medium px-2.5 flex-shrink-0"
              >
                {{
                  isActive(item)
                    ? 'Em vigor'
                    : item.endsAt && new Date(item.endsAt) < now
                      ? 'Encerrada'
                      : 'Agendada'
                }}
              </v-chip>
            </div>

            <!-- Middle: Escopo e Período -->
            <div class="d-flex flex-column ga-1 text-caption text-grey">
              <div class="d-flex align-center ga-1 text-medium-emphasis">
                <v-icon size="14">mdi-map-marker-outline</v-icon>
                <span>
                  {{
                    item.siteId
                      ? siteName(item.siteId)
                      : item.deviceId
                        ? deviceName(item.deviceId)
                        : 'Todos os equipamentos'
                  }}
                </span>
              </div>
              <div class="d-flex align-center ga-1 text-grey-darken-1 mt-0.5">
                <v-icon size="12">mdi-clock-outline</v-icon>
                <span>{{ formatDateTime(item.startsAt) }} — {{ formatDateTime(item.endsAt) }}</span>
              </div>
            </div>

            <!-- Footer Actions -->
            <div class="d-flex align-center justify-end ga-1 pt-2 mt-1 border-t">
              <v-btn
                size="small"
                variant="tonal"
                color="primary"
                prepend-icon="mdi-pencil"
                class="text-caption px-2"
                @click="openDialog(item)"
              >
                Editar
              </v-btn>
              <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
                <v-icon size="18">mdi-delete</v-icon>
              </v-btn>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <MaintenanceWindowDialog v-model="dialog" :window-to-edit="selectedWindow" @saved="onSaved" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref } from 'vue'
import { useSitesStore } from '@/stores/sites'
import { useDevicesStore } from '@/stores/devices'
import { useMaintenanceWindowsStore, type MaintenanceWindow } from '@/stores/maintenanceWindows'
import { useEventsStore } from '@/stores/events'
import MaintenanceWindowDialog from '@/components/MaintenanceWindowDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'

const windowsStore = useMaintenanceWindowsStore()
const sitesStore = useSitesStore()
const devicesStore = useDevicesStore()
const eventsStore = useEventsStore()

let unsubscribe: (() => void) | undefined

const search = ref('')
const dialog = ref(false)
const selectedWindow = ref<MaintenanceWindow | null>(null)
const now = ref(new Date())

const headers = [
  { title: 'Nome', key: 'name' },
  { title: 'Escopo', key: 'scope' },
  { title: 'Período', key: 'period' },
  { title: 'Status', key: 'status', sortable: false },
  { title: 'Ações', key: 'actions', sortable: false, width: '120px' },
]

const feedback = reactive({ visible: false, message: '', color: 'success' })

onMounted(() => {
  void windowsStore.fetchWindows()
  void sitesStore.fetchSites()
  void devicesStore.fetchDevices()
  unsubscribe = eventsStore.onEvent(
    'maintenance_windows:updated',
    () => void windowsStore.fetchWindows()
  )
  setInterval(() => {
    now.value = new Date()
  }, 30_000)
})

onUnmounted(() => {
  unsubscribe?.()
})

function siteName(siteId: number): string {
  return sitesStore.sites.find((s) => s.id === siteId)?.name ?? `Site ${siteId}`
}

function deviceName(deviceId: number): string {
  return devicesStore.devices.find((d) => d.id === deviceId)?.name ?? `Dispositivo ${deviceId}`
}

function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString('pt-BR')
}

function isActive(item: MaintenanceWindow): boolean {
  const start = new Date(item.startsAt)
  const end = new Date(item.endsAt)
  return start <= now.value && now.value <= end
}

function openDialog(windowToEdit?: MaintenanceWindow) {
  selectedWindow.value = windowToEdit ?? null
  dialog.value = true
}

function onSaved() {
  notify(selectedWindow.value ? 'Janela atualizada.' : 'Janela criada.')
  void windowsStore.fetchWindows()
}

async function confirmDelete(id: number) {
  const ok = await confirm({
    title: 'Excluir janela de manutenção',
    message: 'Tem certeza de que deseja excluir esta janela de manutenção?',
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (!ok) return
  const isDeleted = await windowsStore.deleteWindow(id)
  notify(
    isDeleted ? 'Janela excluída.' : windowsStore.error || 'Erro ao excluir janela.',
    isDeleted ? 'success' : 'error'
  )
}

function notify(message: string, color = 'success') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}
</script>
