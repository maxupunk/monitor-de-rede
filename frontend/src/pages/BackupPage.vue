<template>
  <div>
    <PageHeader
      title="Backup"
      subtitle="Cópias de segurança do NetMonitor e dos seus bancos de dados — e de onde restaurá-las quando precisar"
    >
      <template #actions>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-database-export-outline"
          :loading="backupStore.fullDownload != null"
          data-full-backup
          @click="fullBackupDialog = true"
        >
          <span class="hidden-sm-and-down">Baixar backup completo</span>
          <span class="hidden-md-and-up">Backup completo</span>
        </v-btn>
        <v-btn
          v-if="tab === 'protegido'"
          color="primary"
          prepend-icon="mdi-database-plus-outline"
          @click="openDatabaseForm(null)"
        >
          <span class="hidden-sm-and-down">Proteger um banco de dados</span>
          <span class="hidden-md-and-up">Proteger banco</span>
        </v-btn>
        <v-btn v-else color="primary" prepend-icon="mdi-plus" @click="openStorageForm(null)">
          Novo destino
        </v-btn>
      </template>
    </PageHeader>

    <v-tabs v-model="tab" color="primary" class="mb-4">
      <v-tab value="protegido" prepend-icon="mdi-shield-check-outline">O que está protegido</v-tab>
      <v-tab value="destinos" prepend-icon="mdi-database-lock-outline">
        Destinos
        <v-chip size="x-small" color="primary" variant="tonal" class="ml-2">
          {{ storagesStore.storages.length }}
        </v-chip>
      </v-tab>
    </v-tabs>

    <v-window v-model="tab">
      <!-- ── O que está protegido ───────────────────────────────────────── -->
      <v-window-item value="protegido">
        <!-- Primeiro uso: o caminho inteiro, em ordem. -->
        <v-alert
          v-if="storagesStore.loaded && storagesStore.storages.length === 0"
          type="info"
          variant="tonal"
          border="start"
          icon="mdi-map-marker-path"
          class="mb-4"
        >
          <div class="font-weight-bold mb-2">Como funciona</div>
          <ol class="steps text-body-2 mb-3">
            <li>
              <strong>Cadastre um destino</strong> — onde as cópias vão ficar: uma pasta no
              servidor, um NAS por SFTP ou um bucket na nuvem.
            </li>
            <li>
              <strong>Ligue o backup do NetMonitor</strong> — guarda tudo o que você configurou
              aqui.
            </li>
            <li>
              <strong>Proteja seus bancos de dados</strong>, se quiser — PostgreSQL, MySQL ou
              MariaDB.
            </li>
          </ol>
          <v-btn color="info" variant="flat" prepend-icon="mdi-plus" @click="openStorageForm(null)">
            Cadastrar o primeiro destino
          </v-btn>
        </v-alert>

        <v-alert
          v-else-if="failedSummaries.length > 0"
          type="error"
          variant="tonal"
          border="start"
          density="comfortable"
          icon="mdi-database-alert"
          class="mb-4"
        >
          <div class="d-flex flex-wrap align-center justify-space-between ga-2">
            <div>
              <strong>Atenção:</strong> {{ failedSummaries.length }}
              {{ failedSummaries.length === 1 ? 'backup falhou' : 'backups falharam' }} na última
              execução ({{ failedSummaries.join(', ') }}).
            </div>
            <v-btn
              v-if="failedDbCount > 0 && healthFilter !== 'failed'"
              size="small"
              color="error"
              variant="flat"
              @click="healthFilter = 'failed'"
            >
              Filtrar bancos com falha
            </v-btn>
          </div>
        </v-alert>

        <v-alert
          v-else-if="unprotectedDbCount > 0"
          type="warning"
          variant="tonal"
          border="start"
          density="comfortable"
          icon="mdi-shield-off-outline"
          class="mb-4"
        >
          <div class="d-flex flex-wrap align-center justify-space-between ga-2">
            <div>
              {{ unprotectedDbCount }}
              {{
                unprotectedDbCount === 1
                  ? 'banco de dados ainda está sem destino configurado.'
                  : 'bancos de dados ainda estão sem destino configurado.'
              }}
            </div>
            <v-btn
              v-if="healthFilter !== 'unprotected'"
              size="small"
              color="warning"
              variant="flat"
              @click="healthFilter = 'unprotected'"
            >
              Filtrar sem destino
            </v-btn>
          </div>
        </v-alert>

        <div class="section-title">Este sistema</div>
        <v-row class="mb-2">
          <v-col cols="12" md="6" xl="4">
            <BackupPlanCard
              v-if="backupStore.plan"
              title="NetMonitor"
              description="Dispositivos, monitores, alertas, VPN e preferências — tudo o que você configurou aqui."
              icon="mdi-monitor-dashboard"
              color="primary"
              :plan="backupStore.plan"
              :running="backupStore.running || backupStore.plan.running"
              @backup="backupSystem"
              @restore="systemRestoreDialog = true"
              @configure="systemPlanDialog = true"
            >
              <template #menu>
                <v-list-item
                  prepend-icon="mdi-file-download-outline"
                  title="Baixar a configuração"
                  subtitle="Um arquivo .json, para guardar onde quiser"
                  @click="backupStore.exportConfig()"
                />
              </template>
            </BackupPlanCard>
            <v-skeleton-loader v-else type="card" />
          </v-col>
        </v-row>

        <div class="d-flex flex-wrap align-center ga-2 mt-4 mb-3">
          <div class="section-title my-0">Bancos de dados</div>
          <v-chip
            v-if="databasesStore.connections.length > 0"
            size="x-small"
            color="primary"
            variant="tonal"
          >
            {{ filteredConnections.length }}/{{ databasesStore.connections.length }}
          </v-chip>
          <v-spacer></v-spacer>
          <v-btn
            v-if="originGroupsAll.length > 1"
            size="small"
            color="primary"
            :variant="effectiveGroupByOrigin ? 'flat' : 'tonal'"
            prepend-icon="mdi-sitemap-outline"
            @click="groupByOrigin = !effectiveGroupByOrigin"
          >
            Agrupar por origem
          </v-btn>
        </div>

        <v-card
          v-if="databasesStore.connections.length > 0"
          border
          flat
          rounded="lg"
          class="pa-3 mb-4"
        >
          <v-row density="comfortable" align="center">
            <v-col cols="12" md="5">
              <v-text-field
                v-model="dbSearch"
                prepend-inner-icon="mdi-magnify"
                placeholder="Buscar por nome, servidor, banco, destino ou agente…"
                variant="outlined"
                density="compact"
                clearable
                hide-details
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6" md="4">
              <v-select
                v-model="originFilter"
                :items="originFilterItems"
                item-title="title"
                item-value="value"
                label="Acessar a partir de"
                prepend-inner-icon="mdi-transit-connection-variant"
                variant="outlined"
                density="compact"
                hide-details
              ></v-select>
            </v-col>
            <v-col cols="12" sm="6" md="3">
              <div class="d-flex flex-wrap ga-1 justify-sm-end">
                <v-chip
                  size="small"
                  :color="healthFilter === 'all' ? 'primary' : 'secondary'"
                  :variant="healthFilter === 'all' ? 'flat' : 'tonal'"
                  @click="healthFilter = 'all'"
                >
                  Todos
                </v-chip>
                <v-chip
                  v-if="failedDbCount > 0"
                  size="small"
                  color="error"
                  :variant="healthFilter === 'failed' ? 'flat' : 'tonal'"
                  @click="healthFilter = healthFilter === 'failed' ? 'all' : 'failed'"
                >
                  Com falha ({{ failedDbCount }})
                </v-chip>
                <v-chip
                  v-if="unprotectedDbCount > 0"
                  size="small"
                  color="warning"
                  :variant="healthFilter === 'unprotected' ? 'flat' : 'tonal'"
                  @click="healthFilter = healthFilter === 'unprotected' ? 'all' : 'unprotected'"
                >
                  Sem destino ({{ unprotectedDbCount }})
                </v-chip>
              </div>
            </v-col>
          </v-row>
        </v-card>

        <v-card
          v-if="databasesStore.connections.length > 0 && filteredConnections.length === 0"
          border
          flat
          rounded="lg"
          class="pa-6 mb-4 text-center"
        >
          <v-icon size="36" color="info">mdi-filter-off-outline</v-icon>
          <div class="font-weight-bold mt-2">Nenhum banco corresponde ao filtro atual</div>
          <v-btn variant="tonal" color="primary" size="small" class="mt-3" @click="clearDbFilters">
            Limpar filtros
          </v-btn>
        </v-card>

        <div v-for="group in displayedGroups" :key="group.key" class="mb-4">
          <div
            v-if="effectiveGroupByOrigin && filteredConnections.length > 0"
            class="d-flex align-center ga-2 mb-2 mt-1"
          >
            <v-icon size="20" :color="group.originColor">{{ group.originIcon }}</v-icon>
            <span class="text-subtitle-2 font-weight-bold">
              Acessar a partir de: {{ group.originLabel }}
            </span>
            <v-chip size="x-small" :color="group.originColor" variant="tonal">
              {{ group.connections.length }}
              {{ group.connections.length === 1 ? 'banco' : 'bancos' }}
            </v-chip>
          </div>

          <v-row>
            <v-col
              v-for="connection in group.connections"
              :key="connection.id"
              cols="12"
              md="6"
              xl="4"
            >
              <BackupPlanCard
                :title="connection.name"
                :description="describeConnection(connection)"
                :icon="engineInfo(connection.engine).icon"
                :color="engineInfo(connection.engine).color"
                :plan="connection"
                :extra-facts="connectionFacts(connection)"
                restore-label="Cópias e restaurar"
                :running="isRunning(connection.id)"
                :hide-status="activeJob(connection.id) != null"
                @backup="backupDatabase(connection)"
                @restore="openHistory(connection)"
                @configure="openDatabaseForm(connection.id)"
              >
                <DatabaseJobProgress
                  v-if="activeJob(connection.id)"
                  :job="activeJob(connection.id)!"
                  class="mt-3"
                />
                <template #menu>
                  <v-list-item
                    prepend-icon="mdi-lan-check"
                    title="Testar conexão agora"
                    subtitle="Confere alcance, latência e bancos visíveis"
                    :disabled="probingId === connection.id"
                    @click="testConnection(connection)"
                  ></v-list-item>
                  <v-list-item
                    prepend-icon="mdi-history"
                    title="Ver histórico de cópias"
                    subtitle="Baixar .sql.gz ou restaurar uma cópia"
                    @click="openHistory(connection)"
                  ></v-list-item>
                  <v-list-item
                    prepend-icon="mdi-delete-outline"
                    title="Excluir conexão"
                    base-color="error"
                    @click="removeConnection(connection)"
                  ></v-list-item>
                </template>
              </BackupPlanCard>
            </v-col>

            <v-col v-if="group.showAddCard" cols="12" md="6" xl="4">
              <v-card
                border
                flat
                rounded="lg"
                class="add-card h-100 pa-6 d-flex flex-column align-center justify-center text-center"
                @click="openDatabaseForm(null)"
              >
                <div class="d-flex ga-2 mb-3">
                  <v-avatar
                    v-for="item in DATABASE_ENGINES"
                    :key="item.value"
                    :color="item.color"
                    variant="tonal"
                    size="40"
                  >
                    <v-icon size="22">{{ item.icon }}</v-icon>
                  </v-avatar>
                </div>
                <div class="font-weight-bold">Proteger um banco de dados</div>
                <div class="text-body-2 text-high-emphasis">
                  PostgreSQL, MySQL ou MariaDB — sem instalar nada no servidor do banco.
                </div>
              </v-card>
            </v-col>
          </v-row>
        </div>
      </v-window-item>

      <!-- ── Destinos ───────────────────────────────────────────────────── -->
      <v-window-item value="destinos">
        <p class="text-body-2 text-high-emphasis mb-4">
          Onde as cópias ficam. Um destino pode guardar os backups do NetMonitor e de vários bancos
          ao mesmo tempo — cada um na sua pasta.
        </p>

        <v-card
          v-if="storagesStore.loaded && storagesStore.storages.length === 0"
          elevation="2"
          rounded="lg"
          class="pa-6 pa-sm-10 text-center"
        >
          <v-avatar color="primary" variant="tonal" size="72" class="mb-4">
            <v-icon size="40">mdi-database-lock-outline</v-icon>
          </v-avatar>
          <div class="text-h6 font-weight-bold mb-2">Nenhum destino ainda</div>
          <p class="text-body-2 text-high-emphasis mx-auto mb-6" style="max-width: 520px">
            Prefira um lugar fora deste servidor — um NAS ou a nuvem: se o disco daqui falhar, as
            cópias continuam a salvo.
          </p>
          <v-btn
            color="primary"
            size="large"
            prepend-icon="mdi-plus"
            @click="openStorageForm(null)"
          >
            Cadastrar destino
          </v-btn>
        </v-card>

        <v-row v-else>
          <v-col
            v-for="storage in storagesStore.storages"
            :key="storage.id"
            cols="12"
            md="6"
            xl="4"
          >
            <StorageCard
              :storage="storage"
              :contents="contentsOf(storage.id)"
              @explore="openExplorer(storage)"
              @test="testStorage(storage)"
              @edit="openStorageForm(storage.id)"
              @remove="removeStorage(storage)"
            />
          </v-col>
        </v-row>
      </v-window-item>
    </v-window>

    <SystemBackupPlanDialog
      v-model="systemPlanDialog"
      @saved="notify('Backup do NetMonitor salvo.')"
    />
    <SystemRestoreDialog v-model="systemRestoreDialog" />
    <FullBackupDialog v-model="fullBackupDialog" />

    <DatabaseFormDialog
      v-model="databaseFormDialog"
      :connection-id="editingConnectionId"
      @saved="onDatabaseSaved"
      @switch-connection="editingConnectionId = $event"
    />
    <DatabaseHistoryDialog
      v-model="historyDialog"
      :connection="selectedConnection"
      @backup="selectedConnection && backupDatabase(selectedConnection)"
      @restore="openDatabaseRestore"
    />
    <DatabaseRestoreDialog
      v-model="databaseRestoreDialog"
      :backup="restoringBackup"
      :source="selectedConnection"
    />

    <StorageFormDialog
      v-model="storageFormDialog"
      :storage-id="editingStorageId"
      @saved="onStorageSaved"
    />
    <StorageExplorerDialog v-model="explorerDialog" :storage="selectedStorage" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="5000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { DatabaseBackupResponse } from '@/bindings/DatabaseBackupResponse'
import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
import type { DatabaseJobSnapshot } from '@/bindings/DatabaseJobSnapshot'
import type { StorageDestinationDetail } from '@/bindings/StorageDestinationDetail'
import type { StorageDestinationResponse } from '@/bindings/StorageDestinationResponse'
import PageHeader from '@/components/PageHeader.vue'
import BackupPlanCard, { type BackupCardFact } from '@/components/backup/BackupPlanCard.vue'
import FullBackupDialog from '@/components/backup/FullBackupDialog.vue'
import SystemBackupPlanDialog from '@/components/backup/SystemBackupPlanDialog.vue'
import SystemRestoreDialog from '@/components/backup/SystemRestoreDialog.vue'
import DatabaseFormDialog from '@/components/databases/DatabaseFormDialog.vue'
import DatabaseHistoryDialog from '@/components/databases/DatabaseHistoryDialog.vue'
import DatabaseJobProgress from '@/components/databases/DatabaseJobProgress.vue'
import DatabaseRestoreDialog from '@/components/databases/DatabaseRestoreDialog.vue'
import StorageCard, { type DestinationContent } from '@/components/storages/StorageCard.vue'
import StorageExplorerDialog from '@/components/storages/StorageExplorerDialog.vue'
import StorageFormDialog from '@/components/storages/StorageFormDialog.vue'
import { confirm } from '@/composables/useConfirm'
import { useBackupStore } from '@/stores/backup'
import { useDatabasesStore } from '@/stores/databases'
import { useStoragesStore } from '@/stores/storages'
import {
  accessOriginInfo,
  DATABASE_ENGINES,
  databasesLabel,
  engineInfo,
  filterConnections,
  groupConnectionsByOrigin,
  type ConnectionHealthFilter,
} from '@/utils/databasePresentation'
import { formatBytes } from '@/utils/formatters'

const route = useRoute()
const router = useRouter()
const backupStore = useBackupStore()
const databasesStore = useDatabasesStore()
const storagesStore = useStoragesStore()

/** A aba fica na URL: um link para "Destinos" abre direto nela. */
const tab = computed({
  get: () => (route.query.tab === 'destinos' ? 'destinos' : 'protegido'),
  set: (value: string) => void router.replace({ query: { ...route.query, tab: value } }),
})

const systemPlanDialog = ref(false)
const systemRestoreDialog = ref(false)
const fullBackupDialog = ref(false)
const databaseFormDialog = ref(false)
const editingConnectionId = ref<number | null>(null)
const historyDialog = ref(false)
const databaseRestoreDialog = ref(false)
const selectedConnection = ref<DatabaseConnectionResponse | null>(null)
const restoringBackup = ref<DatabaseBackupResponse | null>(null)
const storageFormDialog = ref(false)
const editingStorageId = ref<number | null>(null)
const explorerDialog = ref(false)
const selectedStorage = ref<StorageDestinationResponse | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })
const dbSearch = ref('')
const originFilter = ref<string | null>(null)
const healthFilter = ref<ConnectionHealthFilter>('all')
const groupByOrigin = ref<boolean | null>(null)
const probingId = ref<number | null>(null)
const previousJobStatuses = new Map<string, string>()

watch(
  () => databasesStore.jobs,
  (jobs) => {
    for (const [key, job] of Object.entries(jobs)) {
      const prev = previousJobStatuses.get(key)
      previousJobStatuses.set(key, job.status)
      if (prev === 'running' && job.status === 'success') {
        const connName =
          databasesStore.connections.find((c) => c.id === job.connectionId)?.name ?? 'Banco'
        const what = job.kind === 'restore' ? 'Restauração' : 'Backup'
        notify(`${what} de ${connName} concluído${job.message ? `: ${job.message}` : '.'}`)
      }
    }
  },
  { deep: true }
)

const failedDbCount = computed(
  () => databasesStore.connections.filter((c) => c.lastBackupStatus === 'failed').length
)
const unprotectedDbCount = computed(
  () => databasesStore.connections.filter((c) => c.storageDestinationId == null).length
)
const failedSummaries = computed(() => {
  const names: string[] = []
  if (backupStore.plan?.lastBackupStatus === 'failed') names.push('NetMonitor')
  for (const conn of databasesStore.connections) {
    if (conn.lastBackupStatus === 'failed') names.push(conn.name)
  }
  return names
})

const originGroupsAll = computed(() => groupConnectionsByOrigin(databasesStore.connections))
const effectiveGroupByOrigin = computed(
  () => groupByOrigin.value ?? originGroupsAll.value.length > 1
)
const originFilterItems = computed(() => [
  { title: `Todas as origens (${databasesStore.connections.length})`, value: null },
  ...originGroupsAll.value.map((group) => ({
    title: `${group.origin.label} (${group.connections.length})`,
    value: group.origin.key,
  })),
])

const filteredConnections = computed(() =>
  filterConnections(databasesStore.connections, {
    search: dbSearch.value,
    originKey: originFilter.value,
    health: healthFilter.value,
  })
)

const displayedGroups = computed(() => {
  if (databasesStore.connections.length === 0) {
    return [
      {
        key: 'empty',
        originLabel: '',
        originIcon: '',
        originColor: 'primary',
        connections: [] as DatabaseConnectionResponse[],
        showAddCard: true,
      },
    ]
  }
  if (!effectiveGroupByOrigin.value) {
    return [
      {
        key: 'flat',
        originLabel: '',
        originIcon: '',
        originColor: 'primary',
        connections: filteredConnections.value,
        showAddCard:
          !dbSearch.value.trim() && originFilter.value == null && healthFilter.value === 'all',
      },
    ]
  }
  const grouped = groupConnectionsByOrigin(filteredConnections.value)
  return grouped.map((group, idx) => ({
    key: group.origin.key,
    originLabel: group.origin.label,
    originIcon: group.origin.icon,
    originColor: group.origin.color,
    connections: group.connections,
    showAddCard:
      idx === grouped.length - 1 &&
      !dbSearch.value.trim() &&
      originFilter.value == null &&
      healthFilter.value === 'all',
  }))
})

function clearDbFilters() {
  dbSearch.value = ''
  originFilter.value = null
  healthFilter.value = 'all'
}

/** Quanto tempo um andamento terminado continua no cartão. */
const FINISHED_VISIBLE_MS = 2 * 60 * 1000

onMounted(() => {
  void backupStore.fetchPlan()
  void databasesStore.fetchConnections()
  void storagesStore.fetchStorages()
})

function notify(message: string, color = 'success') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

function failed(err: unknown, fallback: string) {
  notify(err instanceof Error ? err.message : fallback, 'error')
}

// ── NetMonitor ──────────────────────────────────────────────────────────────

async function backupSystem() {
  try {
    const result = await backupStore.runNow()
    const size = result.copy.size != null ? ` (${formatBytes(result.copy.size)})` : ''
    const pruned =
      result.pruned > 0
        ? ` ${result.pruned} ${result.pruned === 1 ? 'cópia antiga removida' : 'cópias antigas removidas'}.`
        : ''
    notify(`Backup do NetMonitor guardado em ${result.storageDestinationName}${size}.${pruned}`)
  } catch (err) {
    failed(err, 'Erro ao fazer o backup do NetMonitor')
  }
}

// ── Bancos de dados ─────────────────────────────────────────────────────────

function describeConnection(connection: DatabaseConnectionResponse): string {
  return `${engineInfo(connection.engine).label} · ${databasesLabel(connection.databases)}`
}

function connectionFacts(connection: DatabaseConnectionResponse): BackupCardFact[] {
  const origin = accessOriginInfo(connection)
  return [
    {
      icon: origin.icon,
      color: origin.color,
      label: 'Acessar a partir de:',
      value: origin.label,
    },
    {
      icon: 'mdi-server-network-outline',
      color: 'primary',
      label: 'Servidor:',
      value: `${connection.host}:${connection.port}`,
    },
  ]
}

async function testConnection(connection: DatabaseConnectionResponse) {
  probingId.value = connection.id
  try {
    const result = await databasesStore.probe({
      id: connection.id,
      engine: connection.engine,
      host: connection.host,
      port: connection.port,
      username: connection.username,
      sslMode: connection.sslMode,
      viaProbeId: connection.viaProbeId,
    })
    notify(
      `${connection.name}: conectado a ${result.version} em ${result.latencyMs} ms (${result.databases.length} ${result.databases.length === 1 ? 'banco visível' : 'bancos visíveis'}).`
    )
  } catch (err) {
    failed(err, `Falha ao conectar em ${connection.name}`)
  } finally {
    probingId.value = null
  }
}

/** O backup em curso — ou o que acabou de terminar, por alguns minutos. */
function activeJob(id: number): DatabaseJobSnapshot | null {
  const job = databasesStore.jobFor('backup', id)
  if (!job) return null
  if (job.status === 'running') return job
  const finished = job.finishedAt ? new Date(job.finishedAt).getTime() : 0
  return Date.now() - finished < FINISHED_VISIBLE_MS ? job : null
}

function isRunning(id: number): boolean {
  return databasesStore.jobFor('backup', id)?.status === 'running'
}

function openDatabaseForm(id: number | null) {
  editingConnectionId.value = id
  databaseFormDialog.value = true
}

function openHistory(connection: DatabaseConnectionResponse) {
  selectedConnection.value = connection
  historyDialog.value = true
}

function openDatabaseRestore(backup: DatabaseBackupResponse) {
  restoringBackup.value = backup
  databaseRestoreDialog.value = true
}

function onDatabaseSaved(saved: DatabaseConnectionResponse, created: boolean) {
  if (!created) notify('Backup do banco salvo.')
  else if (saved.backupEnabled)
    notify(`${saved.name} protegido. O primeiro backup automático sai nos próximos minutos.`)
  else notify(`${saved.name} cadastrado.`)
}

async function backupDatabase(connection: DatabaseConnectionResponse) {
  try {
    await databasesStore.runBackup(connection.id)
    notify(`Backup de ${connection.name} iniciado — acompanhe no cartão.`, 'info')
  } catch (err) {
    failed(err, 'Erro ao iniciar o backup')
  }
}

async function removeConnection(connection: DatabaseConnectionResponse) {
  const ok = await confirm({
    title: 'Excluir conexão',
    message: `"${connection.name}" deixa de ter backup e o histórico dela sai da lista. Os arquivos que já estão no destino continuam lá.`,
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (!ok) return
  if (await databasesStore.remove(connection.id)) notify('Conexão excluída.')
}

// ── Destinos ────────────────────────────────────────────────────────────────

/** De quem cada destino guarda os backups. */
function contentsOf(storageId: number): DestinationContent[] {
  const contents: DestinationContent[] = []
  if (backupStore.plan?.storageDestinationId === storageId) {
    contents.push({ label: 'NetMonitor', icon: 'mdi-monitor-dashboard', color: 'primary' })
  }
  for (const connection of databasesStore.connections) {
    if (connection.storageDestinationId === storageId) {
      const engine = engineInfo(connection.engine)
      contents.push({ label: connection.name, icon: engine.icon, color: engine.color })
    }
  }
  return contents
}

function openStorageForm(id: number | null) {
  editingStorageId.value = id
  storageFormDialog.value = true
}

function openExplorer(storage: StorageDestinationResponse) {
  selectedStorage.value = storage
  explorerDialog.value = true
}

function onStorageSaved(saved: StorageDestinationDetail, created: boolean) {
  if (!created) {
    notify('Destino atualizado.')
    return
  }
  notify(`Destino ${saved.name} cadastrado.`)
  // O próximo passo natural: proteger o NetMonitor nele.
  if (backupStore.plan?.storageDestinationId == null) systemPlanDialog.value = true
}

async function testStorage(storage: StorageDestinationResponse) {
  try {
    const result = await storagesStore.testSaved(storage.id)
    notify(`${storage.name}: ${result.message}`, result.ok ? 'success' : 'error')
  } catch (err) {
    failed(err, 'Erro ao testar a conexão')
  }
}

async function removeStorage(storage: StorageDestinationResponse) {
  const users = contentsOf(storage.id).map((item) => item.label)
  const orphaned = users.length
    ? ` Os backups de ${users.join(', ')} ficam sem destino até você escolher outro.`
    : ''
  const ok = await confirm({
    title: 'Excluir destino',
    message: `"${storage.name}" deixa de receber cópias.${orphaned} Os arquivos que já estão lá continuam onde estão — apague-os antes em "Ver arquivos", se quiser.`,
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (!ok) return
  if (await storagesStore.remove(storage.id)) {
    notify('Destino excluído.')
    if (users.length) {
      void backupStore.fetchPlan()
      void databasesStore.fetchConnections()
    }
  }
}
</script>

<style scoped>
.section-title {
  font-weight: 700;
  font-size: 1rem;
  margin: 4px 0 12px;
}
.steps {
  padding-left: 1.25rem;
  display: grid;
  gap: 4px;
}
.add-card {
  cursor: pointer;
  border-style: dashed;
  border-color: rgba(var(--v-theme-primary), 0.5);
  min-height: 220px;
  transition: border-color 0.15s;
}
.add-card:hover {
  border-color: rgb(var(--v-theme-primary));
}
</style>
